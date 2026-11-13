//! Remote config repo access through the system `git` binary.

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::Context;

use crate::tmp::TempDir;

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

/// Shallow checkout of the config repo in a temp dir, deleted on drop.
#[derive(Debug)]
pub struct Snapshot {
    dir: TempDir,
    pub commit: String,
}

impl Snapshot {
    /// Clones `git_ref` with depth 1 into a fresh temp dir.
    pub fn fetch(url: &str, git_ref: &str) -> anyhow::Result<Snapshot> {
        let dir = TempDir::new("agsync-snapshot")?;
        let dest = dir.path().to_string_lossy().to_string();
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
                "--branch",
                git_ref,
                url,
                &dest,
            ],
            None,
        )?;
        let commit = git(&["rev-parse", "HEAD"], Some(dir.path()))?
            .trim()
            .to_string();
        Ok(Snapshot { dir, commit })
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

fn git(args: &[&str], cwd: Option<&Path>) -> anyhow::Result<String> {
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

    #[test]
    fn ls_remote_and_snapshot() {
        let remote = TempDir::new("agsync-test").unwrap();
        let r = remote.path();
        run(r, &["init", "-q", "-b", "main"]);
        std::fs::write(r.join("agsync.yaml"), "version: 1\n").unwrap();
        run(r, &["add", "."]);
        run(r, &["commit", "-q", "-m", "one"]);
        run(r, &["tag", "-a", "v1", "-m", "tag"]);
        let head = run(r, &["rev-parse", "HEAD"]).trim().to_string();
        let url = r.to_string_lossy().to_string();

        assert_eq!(ls_remote(&url, "main").unwrap(), head);
        assert_eq!(ls_remote(&url, "v1").unwrap(), head);
        assert!(ls_remote(&url, "nope").is_err());

        let snap = Snapshot::fetch(&url, "main").unwrap();
        assert_eq!(snap.commit, head);
        assert!(snap.path().join("agsync.yaml").is_file());
        let path = snap.path().to_path_buf();
        snap.close().unwrap();
        assert!(!path.exists());
    }
}
