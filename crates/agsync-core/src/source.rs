//! Git sources: shallow checkouts cached by commit in `<data>/sources/<name>/<sha>/`,
//! and the build commands that run in them.

use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;

use crate::git;

/// Written in the checkout root only after every build command succeeded.
/// Holds a hash of the commands, so changed commands mean a new build.
pub const BUILT_MARK: &str = ".agsync-built";

/// Where the checkout of `name` at `sha` lives.
pub fn checkout_dir(sources_root: &Path, name: &str, sha: &str) -> PathBuf {
    sources_root.join(name).join(sha)
}

/// True if `commands` already ran in `dir` with success.
pub fn is_built(dir: &Path, commands: &[String]) -> bool {
    std::fs::read_to_string(dir.join(BUILT_MARK)).is_ok_and(|text| text == build_id(commands))
}

fn build_id(commands: &[String]) -> String {
    let joined = commands.join("\n");
    format!("{}\n", crate::hash::hash_bytes(joined.as_bytes()))
}

/// Checks out `sha` into `dir` unless it is already there. Uses a `.tmp` sibling and a
/// rename, so a half-done checkout is never taken for a complete one.
pub fn ensure_checkout(url: &str, git_ref: &str, sha: &str, dir: &Path) -> anyhow::Result<()> {
    if dir.is_dir() {
        return Ok(());
    }
    let tmp = dir.with_extension("tmp");
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp).with_context(|| format!("limpar {}", tmp.display()))?;
    }
    git::checkout(url, git_ref, sha, &tmp)?;
    std::fs::rename(&tmp, dir).with_context(|| format!("renomear para {}", dir.display()))
}

/// Names in `requires` not found in `PATH`; one found only in Windows (WSL interop)
/// says so.
pub fn missing_requires(requires: &[String]) -> Vec<String> {
    requires
        .iter()
        .filter(|name| crate::which::find(name).is_none())
        .map(|name| match crate::which::foreign(name) {
            Some(_) => format!("{name} (achado so no Windows, interop, ignorado)"),
            None => name.clone(),
        })
        .collect()
}

/// One MCP build to run before apply writes the entry.
#[derive(Debug, Clone)]
pub struct BuildJob {
    pub name: String,
    pub repo: String,
    pub git_ref: String,
    pub sha: String,
    pub commands: Vec<String>,
    pub dir: PathBuf,
}

impl BuildJob {
    /// Fresh checkout (an unbuilt leftover is deleted first), then every command in order.
    /// Output goes to a new log in `logs`; on failure the error names that log.
    pub fn run(&self, logs: &Path) -> anyhow::Result<()> {
        if self.dir.is_dir() && !is_built(&self.dir, &self.commands) {
            std::fs::remove_dir_all(&self.dir)
                .with_context(|| format!("limpar checkout {}", self.dir.display()))?;
        }
        ensure_checkout(&self.repo, &self.git_ref, &self.sha, &self.dir)?;
        let log = log_path(logs, &self.name, &self.sha);
        build(&self.dir, &self.commands, &log)
    }
}

/// Runs `commands` in `dir` with the user's environment, then writes [`BUILT_MARK`].
pub fn build(dir: &Path, commands: &[String], log: &Path) -> anyhow::Result<()> {
    if let Some(parent) = log.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut out = File::create(log).with_context(|| format!("criar log {}", log.display()))?;
    for command in commands {
        writeln!(out, "$ {command}")?;
        out.flush()?;
        let status = shell(command)
            .current_dir(dir)
            .stdin(Stdio::null())
            .stdout(out.try_clone()?)
            .stderr(out.try_clone()?)
            .status()
            .with_context(|| format!("executar '{command}'"))?;
        if !status.success() {
            let code = status.code().unwrap_or(-1);
            writeln!(out, "exit {code}")?;
            anyhow::bail!(
                "build falhou em '{command}' (exit {code}); log: {}",
                log.display()
            );
        }
    }
    std::fs::write(dir.join(BUILT_MARK), build_id(commands))
        .with_context(|| format!("marcar build em {}", dir.display()))
}

#[cfg(windows)]
fn shell(command: &str) -> Command {
    use std::os::windows::process::CommandExt;
    let mut cmd = Command::new("cmd");
    cmd.args(["/D", "/S", "/C"])
        .raw_arg(format!("\"{command}\""));
    cmd
}

#[cfg(not(windows))]
fn shell(command: &str) -> Command {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(command);
    cmd
}

fn log_path(logs: &Path, name: &str, sha: &str) -> PathBuf {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let short = &sha[..sha.len().min(12)];
    logs.join(format!("build-{name}-{short}-{millis}.log"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmp::TempDir;

    fn repo(root: &Path) -> (String, String) {
        let r = root.join("remote");
        std::fs::create_dir_all(&r).unwrap();
        let run = |args: &[&str]| {
            let mut all = vec!["-c", "user.name=t", "-c", "user.email=t@t"];
            all.extend_from_slice(args);
            git::git(&all, Some(&r)).unwrap()
        };
        run(&["init", "-q", "-b", "main"]);
        std::fs::write(r.join("server.txt"), "v1").unwrap();
        run(&["add", "."]);
        run(&["commit", "-q", "-m", "one"]);
        let sha = run(&["rev-parse", "HEAD"]).trim().to_string();
        (r.to_string_lossy().to_string(), sha)
    }

    fn job(root: &Path, url: &str, sha: &str, commands: &[&str]) -> BuildJob {
        BuildJob {
            name: "srv".into(),
            repo: url.into(),
            git_ref: "main".into(),
            sha: sha.into(),
            commands: commands.iter().map(|c| c.to_string()).collect(),
            dir: checkout_dir(&root.join("sources"), "srv", sha),
        }
    }

    #[test]
    fn build_ok_marks_and_logs() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let (url, sha) = repo(tmp.path());
        let logs = tmp.path().join("logs");
        let ok = job(
            tmp.path(),
            &url,
            &sha,
            &["echo built> out.txt", "echo second"],
        );
        ok.run(&logs).unwrap();
        assert!(is_built(&ok.dir, &ok.commands));
        let changed = vec!["echo other".to_string()];
        assert!(!is_built(&ok.dir, &changed));
        assert!(ok.dir.join("server.txt").is_file());
        let out = std::fs::read_to_string(ok.dir.join("out.txt")).unwrap();
        assert!(out.starts_with("built"), "{out}");
        let log = std::fs::read_dir(&logs).unwrap().next().unwrap().unwrap();
        let text = std::fs::read_to_string(log.path()).unwrap();
        assert!(text.contains("$ echo built> out.txt"), "{text}");
        assert!(text.contains("$ echo second"), "{text}");
    }

    #[test]
    fn build_failure_leaves_checkout_unbuilt_and_names_log() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let (url, sha) = repo(tmp.path());
        let logs = tmp.path().join("logs");
        let bad = job(
            tmp.path(),
            &url,
            &sha,
            &["echo before", "exit 3", "echo never> never.txt"],
        );
        let err = bad.run(&logs).unwrap_err().to_string();
        assert!(err.contains("exit 3"), "{err}");
        assert!(err.contains("log:"), "{err}");
        assert!(!is_built(&bad.dir, &bad.commands));
        assert!(!bad.dir.join("never.txt").exists());

        std::fs::write(bad.dir.join("leftover"), "x").unwrap();
        let fixed = job(tmp.path(), &url, &sha, &["echo ok"]);
        fixed.run(&logs).unwrap();
        assert!(is_built(&fixed.dir, &fixed.commands));
        assert!(!fixed.dir.join("leftover").exists());
    }

    #[test]
    fn requires_lookup() {
        let wanted = vec!["git".to_string(), "agsync-missing-tool-xyz".to_string()];
        assert_eq!(missing_requires(&wanted), ["agsync-missing-tool-xyz"]);
    }
}
