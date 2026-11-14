//! Linux inside WSL, for real. Skipped (with a message) outside WSL.

#![cfg(target_os = "linux")]

mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::{Env, fake_bin, git, path_with, text};

fn in_wsl(test: &str) -> bool {
    if agsync::host::wsl::Wsl::detect().is_some() {
        return true;
    }
    eprintln!("SKIP {test}: nao e WSL");
    false
}

fn script(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, format!("#!/bin/sh\n{body}\n")).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn codex_env() -> (Env, PathBuf) {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex"]);
    env.path = Some(path_with(&["git", "sh"], std::slice::from_ref(&bin)));
    let log = env.root.path().join("fake.log");
    env.vars = vec![(
        "FAKE_AGENT_LOG".to_string(),
        log.to_string_lossy().to_string(),
    )];
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push("version: 1\nmcps:\n  echo:\n    command: node\n");
    let remote = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &remote, "--profile", "work"]);
    (env, bin)
}

#[test]
fn check_in_wsl_never_calls_notify_send() {
    if !in_wsl("check_in_wsl_never_calls_notify_send") {
        return;
    }
    let (env, bin) = codex_env();
    let called = env.root.path().join("notify-send-called");
    script(
        &bin.join("notify-send"),
        &format!("echo \"$@\" > '{}'", called.display()),
    );
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_agsync"));
    cmd.arg("check");
    env.isolate(&mut cmd);
    cmd.env_remove("AGSYNC_TEST_NOTIFY_LOG")
        .env("DISPLAY", ":0")
        .env("WAYLAND_DISPLAY", "wayland-0");
    let out = cmd.output().unwrap();
    assert!(out.status.success(), "{}", text(&out));
    assert!(env.pending().is_some());
    let log = env.check_log();
    assert!(
        log.contains("notificacao nao mostrada: WSL: so aviso no terminal"),
        "{log}"
    );
    assert!(!called.exists(), "notify-send was called in WSL");
    let hook = env.ok(&["hook"]);
    assert!(hook.contains("1 mudanca pendente"), "{hook}");
}
