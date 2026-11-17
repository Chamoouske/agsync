//! `agsync update`: replaces this binary with the latest release after checking its
//! SHA256, or runs `scoop update agsync` when Scoop installed it.

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::Context;

use crate::paths::Dirs;
use crate::update::{self, Version};

/// Where the release assets live; `AGSYNC_INSTALL_BASE_URL` overrides it, as in the
/// install scripts.
const INSTALL_BASE_URL: &str = "https://github.com/Chamoouske/agsync/releases";

/// README section on Defender and SmartScreen blocking the Windows binary.
#[cfg(windows)]
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
        let question = format!("agsync {current} -> {latest}: rodar `scoop update agsync`?");
        if !crate::prompt::confirm(yes, &question, "agsync update")? {
            return Ok(true);
        }
        return scoop_update(&dirs);
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

/// Runs `scoop update agsync`; a failure exits with Scoop's exit code.
fn scoop_update(dirs: &Dirs) -> anyhow::Result<bool> {
    let scoop = crate::which::require("scoop").map_err(anyhow::Error::msg)?;
    let status = Command::new(scoop)
        .args(["update", "agsync"])
        .status()
        .context("executar `scoop update agsync`")?;
    let code = status.code().unwrap_or(1);
    if code != 0 {
        std::process::exit(code);
    }
    update::clear(dirs)?;
    Ok(true)
}

/// Downloads and checks the release, swaps it in for `exe` and runs it once. A
/// `<exe>.old` left by the last Windows update is deleted first, if it can be.
fn install(exe: &Path, version: Version, asset: &str) -> anyhow::Result<()> {
    let dir = exe.parent().context("pasta do agsync")?;
    let name = exe.file_name().context("nome do agsync")?.to_string_lossy();
    let old = dir.join(format!("{name}.old"));
    let new = dir.join(format!("{name}.new"));
    std::fs::remove_file(&old).ok();
    let bytes = download(version, asset)?;
    std::fs::write(&new, bytes).with_context(|| format!("gravar {}", new.display()))?;
    if let Err(e) = swap(exe, &new, &old) {
        std::fs::remove_file(&new).ok();
        return Err(e);
    }
    if runs(exe) {
        return Ok(());
    }
    restore(exe, &old)
}

/// The release asset, after its SHA256 matched the one in `SHA256SUMS`.
fn download(version: Version, asset: &str) -> anyhow::Result<Vec<u8>> {
    let base = std::env::var("AGSYNC_INSTALL_BASE_URL").unwrap_or(INSTALL_BASE_URL.to_string());
    let proto = protocol(&base)?;
    let url = format!("{}/download/v{version}", base.trim_end_matches('/'));
    let bytes = curl(&format!("{url}/{asset}"), proto)?;
    let sums = curl(&format!("{url}/SHA256SUMS"), proto)?;
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
/// `-q` comes first so no `.curlrc` changes the options.
fn curl(url: &str, proto: &str) -> anyhow::Result<Vec<u8>> {
    let exe = crate::which::require("curl").map_err(anyhow::Error::msg)?;
    let out = Command::new(exe)
        .args(["-q", "--proto", proto, "--proto-redir", proto, "-fsSL"])
        .arg(url)
        .stdin(Stdio::null())
        .output()
        .context("executar curl")?;
    if !out.status.success() {
        anyhow::bail!(
            "baixar {url} falhou: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(out.stdout)
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

/// Atomic rename over the running binary, after `chmod 0755`.
#[cfg(not(windows))]
fn swap(exe: &Path, new: &Path, _old: &Path) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mode = std::fs::Permissions::from_mode(0o755);
    std::fs::set_permissions(new, mode).with_context(|| format!("chmod {}", new.display()))?;
    std::fs::rename(new, exe)
        .with_context(|| format!("mover {} para {}", new.display(), exe.display()))
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
#[cfg(windows)]
fn restore(exe: &Path, old: &Path) -> anyhow::Result<()> {
    std::fs::remove_file(exe).ok();
    let back = std::fs::rename(old, exe);
    let state = match back {
        Ok(()) => "a versao anterior foi restaurada".to_string(),
        Err(e) => format!("restaurar {} falhou: {e}", old.display()),
    };
    anyhow::bail!(
        "{} nao executa depois da troca ({state}); provavelmente o Microsoft Defender ou o SmartScreen bloqueou o binario. Veja {DEFENDER_GUIDE}",
        exe.display()
    )
}

#[cfg(not(windows))]
fn restore(exe: &Path, _old: &Path) -> anyhow::Result<()> {
    anyhow::bail!("{} nao executa depois da troca", exe.display())
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
