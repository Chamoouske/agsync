//! `agsync update`: replaces this binary with the latest release after checking its
//! SHA256, or tells to run `scoop update agsync` when Scoop installed it: Scoop does not
//! update an app that is running, and this process is.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::Context;

use crate::paths::Dirs;
use crate::update::{self, Version};

/// Where the release assets live; `AGSYNC_INSTALL_BASE_URL` overrides it, as in the
/// install scripts.
const INSTALL_BASE_URL: &str = "https://github.com/Chamoouske/agsync/releases";

/// README section on Defender and SmartScreen blocking the Windows binary.
const DEFENDER_GUIDE: &str =
    "https://github.com/Chamoouske/agsync#microsoft-defender-e-smartscreen";

/// Release binary for `os` and `arch` (as in `std::env::consts`); releases only have
/// Windows and Linux x86_64.
fn release_asset(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("windows", "x86_64") => Some("agsync-x86_64-pc-windows-msvc.exe"),
        ("linux", "x86_64") => Some("agsync-x86_64-unknown-linux-musl"),
        _ => None,
    }
}

/// Updates to the latest release after one confirmation.
pub fn update(yes: bool) -> anyhow::Result<bool> {
    let dirs = Dirs::from_env()?;
    let current = Version::current();
    let latest = update::latest()?;
    if latest <= current {
        update::clear(&dirs)?;
        println!("agsync {current} ja e a ultima versao");
        return Ok(true);
    }
    let exe = crate::paths::current_exe()?;
    if from_scoop(&exe, &scoop_roots()) {
        println!(
            "agsync {current} -> {latest}: instalado pelo Scoop, que nao atualiza um agsync em execucao; rode `scoop update agsync`"
        );
        return Ok(true);
    }
    let (os, arch) = (std::env::consts::OS, std::env::consts::ARCH);
    let Some(asset) = release_asset(os, arch) else {
        anyhow::bail!(
            "nao ha binario do agsync para {os} {arch}; so Windows e Linux x86_64. Nada foi baixado"
        );
    };
    let question = format!("atualizar {} de {current} para {latest}?", exe.display());
    if !crate::prompt::confirm(yes, &question, "agsync update")? {
        return Ok(true);
    }
    install(&exe, latest, asset)?;
    update::clear(&dirs)?;
    println!("agsync atualizado para {latest}");
    Ok(true)
}

/// `%SCOOP%` and `%SCOOP_GLOBAL%`: Scoop roots moved away from the defaults.
fn scoop_roots() -> Vec<String> {
    ["SCOOP", "SCOOP_GLOBAL"]
        .iter()
        .filter_map(|var| std::env::var(var).ok())
        .filter(|root| !root.trim().is_empty())
        .collect()
}

/// True for a binary under one of `roots` or under a `scoop\apps` or `scoop\shims`
/// folder, ignoring case and slash direction.
fn from_scoop(exe: &Path, roots: &[String]) -> bool {
    let path = normalize(&exe.to_string_lossy());
    let under_root = roots.iter().any(|root| {
        let prefix = format!("{}\\", normalize(root).trim_end_matches('\\'));
        path.starts_with(&prefix)
    });
    under_root || path.contains("\\scoop\\apps\\") || path.contains("\\scoop\\shims\\")
}

fn normalize(path: &str) -> String {
    path.to_lowercase().replace('/', "\\")
}

/// Downloads and checks the release, swaps it in for `exe` and runs it once; when it
/// does not run, `<exe>.old` goes back in place. On Windows `<exe>.old` is the running
/// binary and stays until the next update deletes it, if it can.
fn install(exe: &Path, version: Version, asset: &str) -> anyhow::Result<()> {
    let dir = exe.parent().context("pasta do agsync")?;
    let name = exe.file_name().context("nome do agsync")?.to_string_lossy();
    let old = dir.join(format!("{name}.old"));
    let new = dir.join(format!("{name}.new-{}", std::process::id()));
    std::fs::remove_file(&old).ok();
    let bytes = download(version, asset)?;
    write_new(&new, &bytes)?;
    if let Err(e) = swap(exe, &new, &old) {
        std::fs::remove_file(&new).ok();
        return Err(e);
    }
    if !runs(exe) {
        return restore(exe, &old);
    }
    if !cfg!(windows) {
        std::fs::remove_file(&old).ok();
    }
    Ok(())
}

/// Writes `bytes` to `path`, which must not exist yet; a partial file is deleted.
fn write_new(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .with_context(|| format!("criar {}", path.display()))?;
    if let Err(e) = file.write_all(bytes).and_then(|()| file.sync_all()) {
        drop(file);
        std::fs::remove_file(path).ok();
        return Err(e).with_context(|| format!("gravar {}", path.display()));
    }
    Ok(())
}

/// The release asset, after its SHA256 matched the one in `SHA256SUMS`.
fn download(version: Version, asset: &str) -> anyhow::Result<Vec<u8>> {
    let base = std::env::var("AGSYNC_INSTALL_BASE_URL").unwrap_or(INSTALL_BASE_URL.to_string());
    let proto = protocol(&base)?;
    let url = format!("{}/download/v{version}", base.trim_end_matches('/'));
    let missing = || {
        format!(
            "release v{version} ainda sem binarios em {url}; tente mais tarde. Nada foi trocado"
        )
    };
    let bytes = curl(&format!("{url}/{asset}"), proto)?.with_context(missing)?;
    let sums = curl(&format!("{url}/SHA256SUMS"), proto)?.with_context(missing)?;
    let sums = String::from_utf8_lossy(&sums);
    let expected =
        expected_hash(&sums, asset).with_context(|| format!("SHA256SUMS nao tem {asset}"))?;
    let actual = crate::hash::hash_bytes(&bytes);
    if actual != expected {
        anyhow::bail!(
            "SHA256 de {asset} nao confere: esperado {expected}, veio {actual}; nada foi trocado"
        );
    }
    Ok(bytes)
}

/// The only protocol curl may use, redirects included: `=https`, or `=file` for a
/// `file://` base (tests). Anything else, `http://` too, is refused.
fn protocol(base: &str) -> anyhow::Result<&'static str> {
    let base = base.to_lowercase();
    if base.starts_with("https://") {
        return Ok("=https");
    }
    if base.starts_with("file://") {
        return Ok("=file");
    }
    anyhow::bail!("AGSYNC_INSTALL_BASE_URL '{base}' recusada: use https:// (ou file://)")
}

/// Body of `url`, kept in memory: nothing lands on disk before the hash is checked.
/// `None` when the file does not exist (HTTP 404, or a missing `file://`). `-q` comes
/// first so no `.curlrc` changes the options.
fn curl(url: &str, proto: &str) -> anyhow::Result<Option<Vec<u8>>> {
    let exe = crate::which::require("curl").map_err(anyhow::Error::msg)?;
    let out = Command::new(exe)
        .args(["-q", "--proto", proto, "--proto-redir", proto, "-fsSL"])
        .arg(url)
        .stdin(Stdio::null())
        .output()
        .context("executar curl")?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    if not_found(out.status.code(), &stderr) {
        return Ok(None);
    }
    if !out.status.success() {
        anyhow::bail!("baixar {url} falhou: {}", stderr.trim());
    }
    Ok(Some(out.stdout))
}

/// curl exit 22 with HTTP 404 (`-f`), or 37: a `file://` that cannot be read.
fn not_found(code: Option<i32>, stderr: &str) -> bool {
    match code {
        Some(22) => stderr.contains(" 404"),
        Some(37) => true,
        _ => false,
    }
}

/// Hash of `asset` in a `SHA256SUMS` text: lines `<hash> <name>` or `<hash> *<name>`.
fn expected_hash(sums: &str, asset: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let [hash, name] = fields[..] else {
            return None;
        };
        let name = name.strip_prefix('*').unwrap_or(name);
        (name == asset).then(|| hash.to_lowercase())
    })
}

/// Windows cannot overwrite a running exe, but can rename it: `exe` becomes `old`, then
/// `new` becomes `exe`.
#[cfg(windows)]
fn swap(exe: &Path, new: &Path, old: &Path) -> anyhow::Result<()> {
    std::fs::rename(exe, old).with_context(|| format!("renomear {}", exe.display()))?;
    if let Err(e) = std::fs::rename(new, exe) {
        std::fs::rename(old, exe).ok();
        return Err(e).with_context(|| format!("mover {} para {}", new.display(), exe.display()));
    }
    Ok(())
}

/// Keeps `exe` as `old` (hard link, else copy), then renames `new` over it
/// atomically, after `chmod 0755`.
#[cfg(not(windows))]
fn swap(exe: &Path, new: &Path, old: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(new, mode).with_context(|| format!("chmod {}", new.display()))?;
    if std::fs::hard_link(exe, old).is_err() {
        std::fs::copy(exe, old).with_context(|| format!("copiar {}", exe.display()))?;
    }
    if let Err(e) = std::fs::rename(new, exe) {
        std::fs::remove_file(old).ok();
        return Err(e).with_context(|| format!("mover {} para {}", new.display(), exe.display()));
    }
    Ok(())
}

fn runs(exe: &Path) -> bool {
    Command::new(exe)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

/// Puts `old` back in place of the new `exe` that did not run.
fn restore(exe: &Path, old: &Path) -> anyhow::Result<()> {
    if cfg!(windows) {
        std::fs::remove_file(exe).ok();
    }
    let state = match std::fs::rename(old, exe) {
        Ok(()) => "a versao anterior foi restaurada".to_string(),
        Err(e) => format!("restaurar {} falhou: {e}", old.display()),
    };
    let hint = if cfg!(windows) {
        format!(
            "; provavelmente o Microsoft Defender ou o SmartScreen bloqueou o binario. Veja {DEFENDER_GUIDE}"
        )
    } else {
        String::new()
    };
    anyhow::bail!(
        "{} nao executa depois da troca ({state}){hint}",
        exe.display()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_hash_with_or_without_star() {
        let sums = "AAA  agsync-x86_64-unknown-linux-musl\n\
                    BBB *agsync-x86_64-pc-windows-msvc.exe\n\
                    CCC  agsync.json\n\
                    garbage\n";
        assert_eq!(
            expected_hash(sums, "agsync-x86_64-unknown-linux-musl").as_deref(),
            Some("aaa")
        );
        assert_eq!(
            expected_hash(sums, "agsync-x86_64-pc-windows-msvc.exe").as_deref(),
            Some("bbb")
        );
        assert_eq!(expected_hash(sums, "agsync.exe"), None);
        assert_eq!(expected_hash("", "agsync.exe"), None);
    }

    #[test]
    fn write_new_never_overwrites() {
        let dir = crate::tmp::TempDir::new("agsync-test").unwrap();
        let path = dir.path().join("agsync.new-1");
        write_new(&path, b"one").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"one");
        assert!(write_new(&path, b"two").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"one");
    }

    #[test]
    fn only_404_and_missing_files_count_as_not_found() {
        let http_404 = "curl: (22) The requested URL returned error: 404";
        let http_500 = "curl: (22) The requested URL returned error: 500";
        assert!(not_found(Some(22), http_404));
        assert!(!not_found(Some(22), http_500));
        assert!(not_found(Some(37), "curl: (37) Couldn't open file"));
        assert!(!not_found(Some(6), "curl: (6) Could not resolve host"));
        assert!(!not_found(Some(0), ""));
        assert!(!not_found(None, ""));
    }

    #[test]
    fn curl_gets_https_or_file_and_nothing_else() {
        assert_eq!(protocol(INSTALL_BASE_URL).unwrap(), "=https");
        assert_eq!(protocol("HTTPS://mirror/x").unwrap(), "=https");
        assert_eq!(protocol("file:///C:/rel").unwrap(), "=file");
        assert!(protocol("http://mirror/x").is_err());
        assert!(protocol("ftp://mirror/x").is_err());
        assert!(protocol("mirror/x").is_err());
    }

    #[test]
    fn only_windows_and_linux_x86_64_have_a_binary() {
        assert_eq!(
            release_asset("windows", "x86_64"),
            Some("agsync-x86_64-pc-windows-msvc.exe")
        );
        assert_eq!(
            release_asset("linux", "x86_64"),
            Some("agsync-x86_64-unknown-linux-musl")
        );
        assert_eq!(release_asset("linux", "aarch64"), None);
        assert_eq!(release_asset("windows", "aarch64"), None);
        assert_eq!(release_asset("macos", "x86_64"), None);
        assert_eq!(release_asset("macos", "aarch64"), None);
    }

    #[test]
    fn scoop_paths_are_found_in_any_case() {
        let found = |path: &str| from_scoop(Path::new(path), &[]);
        assert!(found(r"C:\Users\x\scoop\apps\agsync\current\agsync.exe"));
        assert!(found(r"C:\Users\x\Scoop\Shims\agsync.exe"));
        assert!(found("/c/Users/x/scoop/apps/agsync/agsync.exe"));
        assert!(!found(r"C:\Users\x\AppData\Local\agsync\bin\agsync.exe"));
        assert!(!found(r"C:\Users\x\scoop-tools\apps\agsync.exe"));
        assert!(!found(r"C:\Users\x\scoop\agsync.exe"));
        assert!(!found("/home/x/.local/bin/agsync"));
    }

    #[test]
    fn scoop_roots_from_the_environment_match_by_prefix() {
        let roots = [r"D:\Tools\Pkgs\".to_string(), "e:/global".to_string()];
        let found = |path: &str| from_scoop(Path::new(path), &roots);
        assert!(found(r"d:\tools\pkgs\apps\agsync\current\agsync.exe"));
        assert!(found(r"E:\Global\shims\agsync.exe"));
        assert!(!found(r"D:\Tools\Pkgs-old\agsync.exe"));
        assert!(!found(r"D:\Tools\agsync.exe"));
        assert!(!found(r"E:\globals\agsync.exe"));
    }
}
