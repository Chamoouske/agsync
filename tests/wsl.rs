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

/// A fresh folder in the Windows temp dir, seen from WSL; deleted on drop.
struct WindowsDir(PathBuf);

impl WindowsDir {
    fn new() -> Option<WindowsDir> {
        let host = agsync::host::wsl::Wsl::detect()?;
        let cmd = host.mount_root.join("c/Windows/System32/cmd.exe");
        let out = Command::new(cmd)
            .args(["/d", "/c", "echo %TEMP%"])
            .current_dir(&host.mount_root)
            .output()
            .ok()?;
        let windows = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let out = Command::new("wslpath")
            .args(["-u", &windows])
            .output()
            .ok()?;
        let temp = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = temp.join(format!("agsync-wsl-test-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).ok()?;
        Some(WindowsDir(dir))
    }
}

impl Drop for WindowsDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn programs_on_windows_drives_are_ignored() {
    if !in_wsl("programs_on_windows_drives_are_ignored") {
        return;
    }
    let Some(windows) = WindowsDir::new() else {
        eprintln!("SKIP programs_on_windows_drives_are_ignored: sem pasta temporaria do Windows");
        return;
    };
    let win_bin = windows.0.join("bin");
    let ran = windows.0.join("ran");
    for name in ["claude", "opencode", "winonly-tool"] {
        script(
            &win_bin.join(name),
            &format!("echo {name} >> '{}'", ran.display()),
        );
    }
    script(&win_bin.join("git"), "echo fake windows git; exit 1");
    script(&win_bin.join("hermes.exe"), "exit 1");

    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex"]);
    let mut dirs = vec![win_bin.clone(), bin];
    for tool in ["git", "sh"] {
        dirs.push(
            agsync::which::find(tool)
                .unwrap()
                .parent()
                .unwrap()
                .to_path_buf(),
        );
    }
    env.path = Some(std::env::join_paths(dirs).unwrap());
    let log = env.root.path().join("fake.log");
    env.vars = vec![(
        "FAKE_AGENT_LOG".to_string(),
        log.to_string_lossy().to_string(),
    )];
    git(&env.remote, &["init", "-q", "-b", "main"]);
    let remote = env.remote.to_string_lossy().to_string();
    env.push(&format!(
        "version: 1\nmcps:\n  echo:\n    command: node\n  needs:\n    command: node\n    source: {{git: \"{remote}\", ref: main, requires: [winonly-tool]}}\n"
    ));
    env.ok(&["init", &remote, "--profile", "work"]);

    let status = env.ok(&["status"]);
    for name in ["claude", "opencode"] {
        let line = format!(
            "{name}: {name} achado so no Windows (interop), ignorado: {}",
            win_bin.join(name).display()
        );
        assert!(status.contains(&line), "{status}");
    }
    let hermes = format!(
        "hermes: hermes achado so no Windows (interop), ignorado: {}",
        win_bin.join("hermes.exe").display()
    );
    assert!(status.contains(&hermes), "{status}");
    assert!(
        status.contains(
            "requires ausente no PATH: winonly-tool (achado so no Windows, interop, ignorado)"
        ),
        "{status}"
    );
    assert!(status.contains("codex:\n  mcp echo: adicionar"), "{status}");

    let doctor = env.agsync(&["doctor"]).1;
    let first = doctor.lines().next().unwrap();
    assert!(first.starts_with("info    WSL ("), "{doctor}");
    assert!(
        first.contains(&format!("home Linux ({})", env.home.display())),
        "{first}"
    );
    assert!(
        first.contains("o Windows precisa de instalacao propria"),
        "{first}"
    );
    assert!(
        doctor
            .lines()
            .nth(1)
            .unwrap()
            .starts_with("ok      git version"),
        "{doctor}"
    );
    assert!(!doctor.contains("fake windows git"), "{doctor}");
    env.ok(&["apply", "--yes"]);
    assert!(
        !ran.exists(),
        "a Windows program ran: {:?}",
        std::fs::read_to_string(&ran)
    );
}

/// `agsync <args>` with the real OS scheduler under a test name.
fn real_scheduler(env: &Env, name: &str, args: &[&str]) -> (bool, String) {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_agsync"));
    cmd.args(args);
    env.isolate(&mut cmd);
    cmd.env_remove("AGSYNC_TEST_SCHEDULE_FILE")
        .env("AGSYNC_TEST_SCHEDULE_NAME", name);
    let out = cmd.output().unwrap();
    eprintln!("$ agsync {}\n{}", args.join(" "), text(&out));
    (out.status.success(), text(&out))
}

#[test]
fn without_systemd_and_cron_the_hook_starts_the_check() {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex"]);
    for tool in ["git", "sh"] {
        let exe = agsync::which::find(tool).unwrap();
        std::os::unix::fs::symlink(exe, bin.join(tool)).unwrap();
    }
    env.path = Some(bin.clone().into_os_string());
    let log = env.root.path().join("fake.log");
    env.vars = vec![(
        "FAKE_AGENT_LOG".to_string(),
        log.to_string_lossy().to_string(),
    )];
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push("version: 1\nmcps:\n  echo:\n    command: node\n");
    let remote = env.remote.to_string_lossy().to_string();
    let name = format!("agsync-test-hook-{}", std::process::id());

    let (ok, out) = real_scheduler(
        &env,
        &name,
        &["init", &remote, "--profile", "work", "--yes"],
    );
    assert!(ok, "{out}");
    assert!(
        out.contains("sem `systemctl --user` funcional nem `crontab`"),
        "{out}"
    );
    assert!(out.contains("o `agsync hook` dispara o check"), "{out}");
    let (ok, out) = real_scheduler(&env, &name, &["schedule", "status"]);
    assert!(!ok && out.contains("agendamento ausente"), "{out}");
    assert!(env.pending().is_none());

    env.push("version: 1\nmcps:\n  echo:\n    command: node\n  more:\n    command: node\n");
    let (ok, out) = real_scheduler(&env, &name, &["hook"]);
    assert!(ok && out.is_empty(), "{out}");
    let start = std::time::Instant::now();
    while env.pending().is_none() {
        assert!(start.elapsed().as_secs() < 60, "hook started no check");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let (_, out) = real_scheduler(&env, &name, &["hook"]);
    assert!(out.contains("1 mudanca pendente"), "{out}");
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
