//! Shared end-to-end helpers: temp dirs for every config, local git repos as remotes.
//!
//! No real agent config is ever used: every `claude`, agent CLI and `agsync` run gets a
//! temp home and temp config dirs for Claude Code, Codex, OpenCode and Hermes.

#![allow(dead_code)]

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use agsync::tmp::TempDir;
use serde_json::Value;

pub struct Env {
    pub root: TempDir,
    pub remote: PathBuf,
    pub config: PathBuf,
    pub data: PathBuf,
    pub home: PathBuf,
    pub claude_dir: PathBuf,
    pub codex_dir: PathBuf,
    pub opencode_dir: PathBuf,
    pub hermes_dir: PathBuf,
    pub tmp: PathBuf,
    /// Text that must never show up in agsync output.
    pub forbidden: Vec<String>,
    /// Extra environment for agsync and claude runs.
    pub vars: Vec<(String, String)>,
    /// `PATH` for every run; the inherited one when `None`.
    pub path: Option<OsString>,
}

impl Env {
    pub fn new() -> Env {
        let root = TempDir::new("agsync-e2e").unwrap();
        let dir = |name: &str| {
            let path = root.path().join(name);
            std::fs::create_dir_all(&path).unwrap();
            path
        };
        Env {
            remote: dir("remote"),
            config: dir("config"),
            data: dir("data"),
            home: dir("home"),
            claude_dir: dir("claude"),
            codex_dir: dir("codex"),
            opencode_dir: dir("opencode"),
            hermes_dir: dir("hermes"),
            tmp: dir("tmp"),
            root,
            forbidden: Vec::new(),
            vars: Vec::new(),
            path: None,
        }
    }

    pub fn isolate(&self, cmd: &mut Command) {
        assert!(self.root.path().starts_with(std::env::temp_dir()));
        cmd.env("CLAUDE_CONFIG_DIR", &self.claude_dir)
            .env("CODEX_HOME", &self.codex_dir)
            .env("AGSYNC_OPENCODE_CONFIG_DIR", &self.opencode_dir)
            .env_remove("OPENCODE_CONFIG_DIR")
            .env("HERMES_HOME", &self.hermes_dir)
            .env("HOME", &self.home)
            .env("USERPROFILE", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("AGSYNC_CONFIG_DIR", &self.config)
            .env("AGSYNC_DATA_DIR", &self.data)
            .env("TEMP", &self.tmp)
            .env("TMP", &self.tmp)
            .env("TMPDIR", &self.tmp)
            .stdin(Stdio::null());
        if let Some(path) = &self.path {
            cmd.env("PATH", path);
        }
        for (key, value) in &self.vars {
            cmd.env(key, value);
        }
    }

    pub fn agsync(&self, args: &[&str]) -> (bool, String) {
        self.agsync_with_input(args, None)
    }

    /// Runs agsync; `input` is written to its stdin when given.
    pub fn agsync_with_input(&self, args: &[&str], input: Option<&str>) -> (bool, String) {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_agsync"));
        cmd.args(args);
        self.isolate(&mut cmd);
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        if input.is_some() {
            cmd.stdin(Stdio::piped());
        }
        let mut child = cmd.spawn().unwrap();
        if let Some(input) = input {
            let mut stdin = child.stdin.take().unwrap();
            stdin.write_all(input.as_bytes()).unwrap();
        }
        let out = child.wait_with_output().unwrap();
        let text = text(&out);
        for secret in &self.forbidden {
            assert!(!text.contains(secret), "secret leaked in output:\n{text}");
        }
        eprintln!("$ agsync {}\n{text}", args.join(" "));
        (out.status.success(), text)
    }

    pub fn ok(&self, args: &[&str]) -> String {
        let (ok, text) = self.agsync(args);
        assert!(ok, "agsync {args:?} failed:\n{text}");
        text
    }

    pub fn claude(&self, args: &[&str]) -> String {
        let mut cmd = Command::new("claude");
        cmd.args(args);
        self.isolate(&mut cmd);
        let out = cmd.output().unwrap();
        assert!(out.status.success(), "claude {args:?}: {}", text(&out));
        text(&out)
    }

    pub fn push(&self, yaml: &str) {
        std::fs::write(self.remote.join("agsync.yaml"), yaml).unwrap();
        git(&self.remote, &["add", "-A"]);
        git(&self.remote, &["commit", "-q", "-m", "update"]);
    }

    pub fn servers(&self) -> serde_json::Map<String, Value> {
        let text = std::fs::read_to_string(self.claude_dir.join(".claude.json")).unwrap();
        let root: Value = serde_json::from_str(&text).unwrap();
        root["mcpServers"].as_object().cloned().unwrap_or_default()
    }

    pub fn args_of(&self, name: &str) -> Value {
        self.servers()[name]["args"].clone()
    }

    pub fn state(&self) -> String {
        std::fs::read_to_string(self.data.join("state.json")).unwrap()
    }

    pub fn skill(&self, name: &str) -> PathBuf {
        self.claude_dir.join("skills").join(name)
    }
}

pub fn claude_missing(test: &str) -> bool {
    if agsync::which::find("claude").is_some() {
        return false;
    }
    eprintln!("SKIP {test}: `claude` nao esta no PATH");
    true
}

pub fn text(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

pub fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "tag.gpgsign=false",
        ])
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {}", text(&out));
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

pub fn count(dir: &Path) -> usize {
    std::fs::read_dir(dir).map(|d| d.count()).unwrap_or(0)
}
