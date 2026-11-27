//! Remote repo access through the system `git` binary.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::Context;

use crate::tmp::TempDir;

/// True for a full 40-character hex commit SHA.
pub fn is_full_sha(text: &str) -> bool {
    text.len() == 40 && text.chars().all(|c| c.is_ascii_hexdigit())
}

/// True for something that looks like an abbreviated SHA (7 to 39 hex chars).
pub fn is_short_sha(text: &str) -> bool {
    (7..40).contains(&text.len()) && text.chars().all(|c| c.is_ascii_hexdigit())
}

/// Commit SHA for `git_ref`: a full SHA is returned as is, else asked to `git ls-remote`.
pub fn resolve(url: &str, git_ref: &str) -> anyhow::Result<String> {
    refuse_option(url, git_ref)?;
    if is_full_sha(git_ref) {
        return Ok(git_ref.to_lowercase());
    }
    if is_short_sha(git_ref) {
        anyhow::bail!("ref '{git_ref}' parece SHA abreviado; use o SHA completo (40 caracteres)");
    }
    ls_remote(url, git_ref)
}

/// Commit SHA that `git_ref` (branch or tag) points to on the remote.
pub fn ls_remote(url: &str, git_ref: &str) -> anyhow::Result<String> {
    refuse_option(url, git_ref)?;
    let peeled = format!("{git_ref}^{{}}");
    let args = ["ls-remote", "--end-of-options", url, git_ref, &peeled];
    let out = network(&args, None)?;
    let mut fallback = None;
    for line in out.lines() {
        let Some((sha, name)) = line.split_once('\t') else {
            continue;
        };
        if name.ends_with("^{}") {
            return Ok(sha.to_string());
        }
        fallback.get_or_insert_with(|| sha.to_string());
    }
    fallback.with_context(|| format!("ref '{git_ref}' nao encontrada em {url}"))
}

/// Shallow checkout of exactly `sha` into `dest` (created; must not hold a repo yet).
///
/// Fetches the SHA directly. If the server refuses that, clones `git_ref` shallowly
/// and checks that it still points to `sha`.
pub fn checkout(url: &str, git_ref: &str, sha: &str, dest: &Path) -> anyhow::Result<()> {
    refuse_option(url, git_ref)?;
    std::fs::create_dir_all(dest).with_context(|| format!("criar {}", dest.display()))?;
    let fetched = fetch_sha(url, sha, dest);
    if fetched.is_ok() {
        return Ok(());
    }
    std::fs::remove_dir_all(dest).with_context(|| format!("limpar {}", dest.display()))?;
    if is_full_sha(git_ref) {
        return fetched.context("o servidor recusou fetch por SHA");
    }
    clone_ref(url, git_ref, sha, dest)
}

fn fetch_sha(url: &str, sha: &str, dest: &Path) -> anyhow::Result<()> {
    git(&["init", "-q"], Some(dest))?;
    git(&["config", "core.autocrlf", "false"], Some(dest))?;
    let args = [
        "fetch",
        "-q",
        "--depth",
        "1",
        "--no-tags",
        "--end-of-options",
        url,
        sha,
    ];
    network(&args, Some(dest))?;
    git(&["checkout", "-q", "FETCH_HEAD"], Some(dest))?;
    Ok(())
}

fn clone_ref(url: &str, git_ref: &str, sha: &str, dest: &Path) -> anyhow::Result<()> {
    let dest_text = dest.to_string_lossy().to_string();
    network(
        &[
            "-c",
            "core.autocrlf=false",
            "clone",
            "--quiet",
            "--depth",
            "1",
            "--single-branch",
            "--no-tags",
            "--config",
            "core.autocrlf=false",
            "--branch",
            git_ref,
            "--end-of-options",
            url,
            &dest_text,
        ],
        None,
    )?;
    let head = git(&["rev-parse", "HEAD"], Some(dest))?.trim().to_string();
    if head != sha {
        anyhow::bail!(
            "'{git_ref}' mudou durante a leitura (esperado {sha}, veio {head}); rode o comando de novo"
        );
    }
    Ok(())
}

/// Refuses a url or ref that `git` would read as an option (`--upload-pack=...`
/// runs a command).
fn refuse_option(url: &str, git_ref: &str) -> anyhow::Result<()> {
    if url.starts_with('-') {
        anyhow::bail!("url git '{url}' comeca com -; recusada");
    }
    if git_ref.starts_with('-') {
        anyhow::bail!("ref '{git_ref}' comeca com -; recusada");
    }
    Ok(())
}

/// [`git`] for commands that reach a remote: the `ext::` transport, which runs a
/// command, stays off whatever the user's git config says.
///
/// Ssh runs in batch mode too: a key with a passphrase outside the ssh-agent fails
/// fast, with a hint, instead of asking for the passphrase or hanging.
pub fn network(args: &[&str], cwd: Option<&Path>) -> anyhow::Result<String> {
    network_with(args, cwd, &SshSetup::read(cwd))
}

fn network_with(args: &[&str], cwd: Option<&Path>, ssh: &SshSetup) -> anyhow::Result<String> {
    let mut all = vec!["-c", "protocol.ext.allow=never"];
    all.extend_from_slice(args);
    let command = ssh.batch();
    run(&all, cwd, command.as_deref()).map_err(|e| match ssh_hint(&format!("{e:#}")) {
        Some(hint) => anyhow::anyhow!("{e:#}; {hint}"),
        None => e,
    })
}

/// The user's ssh setup for git: environment first, then git config.
#[derive(Debug, Default)]
struct SshSetup {
    env_command: Option<String>,
    env_program: bool,
    env_variant: Option<String>,
    command: Option<String>,
    variant: Option<String>,
}

impl SshSetup {
    fn read(cwd: Option<&Path>) -> SshSetup {
        let pattern = r"^(core\.sshcommand|ssh\.variant)$";
        let config = git(&["config", "--get-regexp", pattern], cwd).unwrap_or_default();
        let value = |key: &str| {
            config
                .lines()
                .filter_map(|line| line.split_once(' '))
                .filter(|(k, _)| *k == key)
                .map(|(_, v)| v.to_string())
                .next_back()
        };
        SshSetup {
            env_command: std::env::var("GIT_SSH_COMMAND").ok(),
            env_program: std::env::var_os("GIT_SSH").is_some(),
            env_variant: std::env::var("GIT_SSH_VARIANT").ok(),
            command: value("core.sshcommand"),
            variant: value("ssh.variant"),
        }
    }

    /// `GIT_SSH_COMMAND` for a network call: the user's ssh command (env, then
    /// `core.sshCommand`, else `ssh`) with `-o BatchMode=yes` appended. `None` keeps
    /// git's own choice when the ssh may not be OpenSSH: a `GIT_SSH` program, plink,
    /// or an ssh variant other than `ssh`.
    fn batch(&self) -> Option<String> {
        let base = match given(&self.env_command) {
            Some(command) => command,
            None if self.env_program => return None,
            None => given(&self.command).unwrap_or_else(|| "ssh".to_string()),
        };
        let variant = given(&self.env_variant)
            .or_else(|| given(&self.variant))
            .unwrap_or_else(|| "auto".to_string())
            .to_lowercase();
        let openssh = match variant.as_str() {
            "ssh" => true,
            "auto" => !is_plink(&base),
            _ => false,
        };
        openssh.then(|| format!("{base} -o BatchMode=yes"))
    }
}

fn given(text: &Option<String>) -> Option<String> {
    let text = text.as_deref()?.trim();
    (!text.is_empty()).then(|| text.to_string())
}

/// True when the program of an ssh command is plink or tortoiseplink.
fn is_plink(command: &str) -> bool {
    let program = match command.strip_prefix(['"', '\'']) {
        Some(rest) => rest.split(['"', '\'']).next(),
        None => command.split_whitespace().next(),
    };
    let file = program
        .unwrap_or("")
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("");
    let file = file.to_lowercase();
    let name = file.strip_suffix(".exe").unwrap_or(&file);
    name == "plink" || name == "tortoiseplink"
}

/// Hint for an ssh failure caused by running without prompts.
pub fn ssh_hint(error: &str) -> Option<&'static str> {
    let text = error.to_lowercase();
    if text.contains("permission denied (publickey") {
        return Some(
            "o agsync roda o ssh sem prompt (BatchMode=yes): carregue a chave no ssh-agent \
             (`ssh-add <chave>`) ou use uma chave sem passphrase; teste com `ssh -o BatchMode=yes -T <host>`",
        );
    }
    if text.contains("host key verification failed") {
        return Some(
            "o agsync roda o ssh sem prompt (BatchMode=yes): aceite a chave do host antes, \
             com `ssh -T <host>` uma vez no terminal",
        );
    }
    None
}

/// Checkout of the config repo: a temp dir deleted on drop, or a cache entry kept.
#[derive(Debug)]
pub struct Snapshot {
    dir: Option<TempDir>,
    path: PathBuf,
    pub commit: String,
}

impl Snapshot {
    /// Checks out exactly `sha` (resolved from `git_ref`) into a fresh temp dir.
    pub fn fetch(url: &str, git_ref: &str, sha: &str) -> anyhow::Result<Snapshot> {
        let dir = TempDir::new("agsync-snapshot")?;
        checkout(url, git_ref, sha, dir.path())?;
        Ok(Snapshot {
            path: dir.path().to_path_buf(),
            dir: Some(dir),
            commit: sha.to_string(),
        })
    }

    /// Files of `url` at `sha` from the cache in `cache`, checked out only on a miss
    /// (see [`crate::snapshot_cache`]). Closing it keeps the entry.
    pub fn cached(cache: &Path, url: &str, git_ref: &str, sha: &str) -> anyhow::Result<Snapshot> {
        refuse_option(url, git_ref)?;
        let path = crate::snapshot_cache::open(cache, url, git_ref, sha)?;
        Ok(Snapshot {
            dir: None,
            path,
            commit: sha.to_string(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Deletes a temp snapshot, reporting failure; a cached one stays.
    pub fn close(self) -> anyhow::Result<()> {
        let Some(dir) = self.dir else {
            return Ok(());
        };
        let path = self.path.display().to_string();
        dir.close()
            .with_context(|| format!("apagar snapshot {path}"))
    }
}

/// Runs `git` and returns stdout; the error carries git's stderr.
///
/// Git never prompts for credentials here (`GIT_TERMINAL_PROMPT=0`, no Git
/// Credential Manager dialog, no ssh askpass): a missing credential fails fast instead of hanging.
/// The binary is the one [`crate::which::find`] picks, never a Windows `git` seen
/// from WSL.
pub fn git(args: &[&str], cwd: Option<&Path>) -> anyhow::Result<String> {
    run(args, cwd, None)
}

fn run(args: &[&str], cwd: Option<&Path>, ssh: Option<&str>) -> anyhow::Result<String> {
    let exe = crate::which::require("git").map_err(anyhow::Error::msg)?;
    let mut cmd = Command::new(exe);
    cmd.args(args)
        .stdin(Stdio::null())
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "never")
        .env("SSH_ASKPASS_REQUIRE", "never");
    if let Some(ssh) = ssh {
        cmd.env("GIT_SSH_COMMAND", ssh);
    }
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let out = cmd.output().context("executar git (esta no PATH?)")?;
    if !out.status.success() {
        let command = args
            .iter()
            .find(|a| !a.starts_with('-') && !a.contains('='))
            .unwrap_or(&"");
        anyhow::bail!(
            "git {command} falhou: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(dir: &Path, args: &[&str]) -> String {
        let mut all = vec![
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "tag.gpgsign=false",
        ];
        all.extend_from_slice(args);
        git(&all, Some(dir)).unwrap()
    }

    fn remote_with_two_commits() -> (TempDir, String, String, String) {
        let remote = TempDir::new("agsync-test").unwrap();
        let r = remote.path();
        run(r, &["init", "-q", "-b", "main"]);
        std::fs::write(r.join("agsync.yaml"), "version: 1\n").unwrap();
        run(r, &["add", "."]);
        run(r, &["commit", "-q", "-m", "one"]);
        run(r, &["tag", "-a", "v1", "-m", "tag"]);
        let first = run(r, &["rev-parse", "HEAD"]).trim().to_string();
        std::fs::write(r.join("agsync.yaml"), "version: 1\nprofiles: []\n").unwrap();
        run(r, &["commit", "-q", "-am", "two"]);
        let head = run(r, &["rev-parse", "HEAD"]).trim().to_string();
        let url = r.to_string_lossy().to_string();
        (remote, url, first, head)
    }

    #[test]
    fn resolve_refs() {
        let (_remote, url, first, head) = remote_with_two_commits();
        assert_eq!(resolve(&url, "main").unwrap(), head);
        assert_eq!(resolve(&url, "v1").unwrap(), first);
        assert_eq!(resolve(&url, &first.to_uppercase()).unwrap(), first);
        assert!(resolve(&url, "nope").is_err());
        let short = resolve(&url, &first[..10]).unwrap_err().to_string();
        assert!(short.contains("abreviado"), "{short}");
    }

    #[test]
    fn snapshot_holds_exact_sha() {
        let (_remote, url, first, head) = remote_with_two_commits();
        let snap = Snapshot::fetch(&url, &first, &first).unwrap();
        let text = std::fs::read_to_string(snap.path().join("agsync.yaml")).unwrap();
        assert_eq!(text, "version: 1\n");
        let got = git(&["rev-parse", "HEAD"], Some(snap.path())).unwrap();
        assert_eq!(got.trim(), first);
        let path = snap.path().to_path_buf();
        snap.close().unwrap();
        assert!(!path.exists());

        let snap = Snapshot::fetch(&url, "main", &head).unwrap();
        assert_eq!(snap.commit, head);
    }

    #[test]
    fn cached_snapshot_survives_close() {
        let (remote, url, _first, head) = remote_with_two_commits();
        let cache = remote.path().join(".cache");
        let snap = Snapshot::cached(&cache, &url, "main", &head).unwrap();
        assert_eq!(snap.commit, head);
        let path = snap.path().to_path_buf();
        snap.close().unwrap();
        let text = std::fs::read_to_string(path.join("agsync.yaml")).unwrap();
        assert_eq!(
            text,
            "version: 1
profiles: []
"
        );
        let again = Snapshot::cached(&cache, &url, "main", &head).unwrap();
        assert_eq!(again.path(), path);
    }

    #[test]
    fn git_runs_without_credential_prompts() {
        let seen = git(
            &[
                "-c",
                "alias.agsyncenv=!echo $GIT_TERMINAL_PROMPT $GCM_INTERACTIVE",
                "agsyncenv",
            ],
            None,
        )
        .unwrap();
        assert_eq!(seen.trim(), "0 never");
    }

    #[test]
    fn batch_mode_is_appended_to_the_users_ssh_command() {
        let text = |t: &str| Some(t.to_string());
        let user = SshSetup {
            env_command: text("ssh -i ~/.ssh/k"),
            env_program: true,
            command: text("myssh"),
            ..SshSetup::default()
        };
        assert_eq!(user.batch().unwrap(), "ssh -i ~/.ssh/k -o BatchMode=yes");
        let config = SshSetup {
            command: text("myssh\n"),
            ..SshSetup::default()
        };
        assert_eq!(config.batch().unwrap(), "myssh -o BatchMode=yes");
        let empty = SshSetup {
            env_command: text(""),
            ..SshSetup::default()
        };
        assert_eq!(empty.batch().unwrap(), "ssh -o BatchMode=yes");
        let program = SshSetup {
            env_program: true,
            command: text("myssh"),
            ..SshSetup::default()
        };
        assert!(program.batch().is_none());
    }

    #[test]
    fn batch_mode_skips_ssh_that_is_not_openssh() {
        let text = |t: &str| Some(t.to_string());
        let with = |command: &str, variant: Option<&str>| SshSetup {
            command: text(command),
            variant: variant.map(String::from),
            ..SshSetup::default()
        };
        assert!(with("plink -batch", None).batch().is_none());
        assert!(
            with("\"C:\\Program Files\\PuTTY\\PLINK.EXE\"", None)
                .batch()
                .is_none()
        );
        assert!(with("/usr/bin/tortoiseplink", None).batch().is_none());
        assert!(with("myssh", Some("putty")).batch().is_none());
        assert!(with("myssh", Some("simple")).batch().is_none());
        assert!(with("plink", Some("ssh")).batch().is_some());
        assert!(with("ssh", Some("auto")).batch().is_some());
        let env = SshSetup {
            env_variant: text("plink"),
            variant: text("ssh"),
            ..SshSetup::default()
        };
        assert!(env.batch().is_none());
    }

    #[test]
    fn ssh_without_agent_fails_fast_with_hint() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let dir = tmp.path();
        let mark = dir.join("prompted").to_string_lossy().replace('\\', "/");
        let fake = dir.join("fake-ssh");
        let script = format!(
            "case \"$*\" in *BatchMode=yes*) echo 'git@h: Permission denied (publickey).' >&2; exit 255;; esac\ntouch '{mark}'\nexit 1\n"
        );
        std::fs::write(&fake, script).unwrap();
        let fake_text = fake.to_string_lossy().replace('\\', "/");
        git(&["init", "-q"], Some(dir)).unwrap();
        git(
            &["config", "core.sshCommand", &format!("sh '{fake_text}'")],
            Some(dir),
        )
        .unwrap();
        let args = ["ls-remote", "ssh://git@agsync.invalid/x.git"];
        let ssh = SshSetup {
            env_command: None,
            env_program: false,
            env_variant: None,
            ..SshSetup::read(Some(dir))
        };
        let error = network_with(&args, Some(dir), &ssh)
            .unwrap_err()
            .to_string();
        assert!(error.contains("Permission denied (publickey)"), "{error}");
        assert!(error.contains("ssh-add"), "{error}");
        assert!(!dir.join("prompted").exists());
    }

    #[test]
    fn refuses_url_or_ref_read_as_option() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let mark = tmp.path().join("ran");
        let mark_text = mark.to_string_lossy().replace('\\', "/");
        let evil = format!("--upload-pack=touch '{mark_text}'");
        let full = "a".repeat(40);
        let errors = [
            resolve(&evil, "main").unwrap_err(),
            resolve(&evil, &full).unwrap_err(),
            ls_remote(&evil, "main").unwrap_err(),
            checkout(&evil, "main", &full, &tmp.path().join("co")).unwrap_err(),
            Snapshot::fetch(&evil, "main", &full).unwrap_err(),
            Snapshot::cached(&tmp.path().join("cache"), &evil, "main", &full).unwrap_err(),
            resolve("https://x/r.git", "--upload-pack=x").unwrap_err(),
        ];
        for e in errors {
            assert!(e.to_string().contains("comeca com -"), "{e}");
        }
        assert!(!mark.exists());
        assert!(!tmp.path().join("co").exists());
        assert!(!tmp.path().join("cache").exists());
    }

    #[test]
    fn options_after_end_of_options_are_urls() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let mark = tmp.path().join("ran");
        let mark_text = mark.to_string_lossy().replace('\\', "/");
        let evil = format!("--upload-pack=touch '{mark_text}'");
        let args = ["ls-remote", "--end-of-options", &evil, "main"];
        assert!(network(&args, Some(tmp.path())).is_err());
        assert!(!mark.exists());
    }

    #[test]
    fn network_calls_turn_off_the_ext_transport() {
        let seen = network(
            &[
                "-c",
                "alias.agsyncext=!git config protocol.ext.allow",
                "agsyncext",
            ],
            None,
        )
        .unwrap();
        assert_eq!(seen.trim(), "never");
    }

    #[test]
    fn clone_fallback_checks_head() {
        let (_remote, url, first, head) = remote_with_two_commits();
        let tmp = TempDir::new("agsync-test").unwrap();
        clone_ref(&url, "main", &head, &tmp.path().join("ok")).unwrap();
        let moved = clone_ref(&url, "main", &first, &tmp.path().join("moved"))
            .unwrap_err()
            .to_string();
        assert!(moved.contains("rode o comando de novo"), "{moved}");
    }
}
