//! Remote repo access through the system `git` binary.

use std::path::Path;
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
    let peeled = format!("{git_ref}^{{}}");
    let out = git(&["ls-remote", url, git_ref, &peeled], None)?;
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
    git(
        &["fetch", "-q", "--depth", "1", "--no-tags", url, sha],
        Some(dest),
    )?;
    git(&["checkout", "-q", "FETCH_HEAD"], Some(dest))?;
    Ok(())
}

fn clone_ref(url: &str, git_ref: &str, sha: &str, dest: &Path) -> anyhow::Result<()> {
    let dest_text = dest.to_string_lossy().to_string();
    git(
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

/// Shallow checkout of the config repo in a temp dir, deleted on drop.
#[derive(Debug)]
pub struct Snapshot {
    dir: TempDir,
    pub commit: String,
}

impl Snapshot {
    /// Checks out exactly `sha` (resolved from `git_ref`) into a fresh temp dir.
    pub fn fetch(url: &str, git_ref: &str, sha: &str) -> anyhow::Result<Snapshot> {
        let dir = TempDir::new("agsync-snapshot")?;
        checkout(url, git_ref, sha, dir.path())?;
        Ok(Snapshot {
            dir,
            commit: sha.to_string(),
        })
    }

    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Deletes the snapshot, reporting failure.
    pub fn close(self) -> anyhow::Result<()> {
        let path = self.dir.path().display().to_string();
        self.dir
            .close()
            .with_context(|| format!("apagar snapshot {path}"))
    }
}

/// Runs `git` and returns stdout; the error carries git's stderr.
pub fn git(args: &[&str], cwd: Option<&Path>) -> anyhow::Result<String> {
    let mut cmd = Command::new("git");
    cmd.args(args).stdin(Stdio::null());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let out = cmd.output().context("executar git (esta no PATH?)")?;
    if !out.status.success() {
        anyhow::bail!(
            "git {} falhou: {}",
            args.first().unwrap_or(&""),
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
