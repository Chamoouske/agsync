//! What Linux does for real, beyond the unit tests: file modes of `secrets.env` and
//! `backups`, skills as symlinks, builds through `sh -c`, and the shell hook run by
//! real interactive bash, zsh and fish. Every run has a temp home.

#![cfg(target_os = "linux")]

mod common;

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use common::{Env, fake_bin, git, path_with, text};

fn mode(path: &Path) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

fn source_repo(env: &Env) -> String {
    let dir = env.root.path().join("source");
    let skill = dir.join("skills").join("linux-skill");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nname: linux-skill\ndescription: test\n---\nv1\n",
    )
    .unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "one"]);
    dir.to_string_lossy().to_string()
}

fn codex_env() -> Env {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex"]);
    env.path = Some(path_with(&["git", "sh", "uname"], &[bin]));
    let log = env.root.path().join("fake.log");
    env.vars = vec![(
        "FAKE_AGENT_LOG".to_string(),
        log.to_string_lossy().to_string(),
    )];
    env
}

#[test]
fn apply_uses_private_modes_symlinks_and_sh() {
    let env = codex_env();
    let url = source_repo(&env);
    let yaml = format!(
        r#"version: 1
mcps:
  built:
    command: node
    args: ["{{dir}}/shell.txt"]
    env: {{TOKEN: "${{E2E_LINUX_FILE_SECRET}}"}}
    source:
      git: "{url}"
      ref: main
      build: ['printf "%s|%s" "$0" "$(uname -s)" > shell.txt']
skills:
  - git: "{url}"
    ref: main
    path: skills/linux-skill
"#
    );
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push(&yaml);
    let remote = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &remote, "--profile", "work"]);
    let (ok, out) = env.agsync_with_input(
        &["secret", "set", "E2E_LINUX_FILE_SECRET"],
        Some("linux-secret\n"),
    );
    assert!(ok, "{out}");
    assert_eq!(mode(&env.config.join("secrets.env")), 0o600);

    env.ok(&["apply", "--yes"]);
    let backups = env.data.join("backups");
    assert_eq!(mode(&backups), 0o700);
    let sha = git(Path::new(&url), &["rev-parse", "HEAD"]);
    let checkout = env.data.join("sources").join("built").join(&sha);
    assert_eq!(
        std::fs::read_to_string(checkout.join("shell.txt")).unwrap(),
        "sh|Linux"
    );

    let link = env.home.join(".agents").join("skills").join("linux-skill");
    let meta = std::fs::symlink_metadata(&link).unwrap();
    assert!(meta.is_symlink(), "{}", link.display());
    let target = std::fs::read_link(&link).unwrap();
    assert!(target.starts_with(env.data.join("skills")), "{target:?}");
    assert!(link.join("SKILL.md").is_file());
    let state = env.state();
    assert!(state.contains("\"mode\": \"symlink\""), "{state}");

    let doctor = env.agsync(&["doctor"]).1;
    assert!(
        doctor.contains("secrets.env acessivel so pelo usuario"),
        "{doctor}"
    );
    assert!(
        doctor.contains("backups acessivel so pelo usuario"),
        "{doctor}"
    );
    std::fs::set_permissions(&backups, std::fs::Permissions::from_mode(0o755)).unwrap();
    let doctor = env.agsync(&["doctor"]).1;
    assert!(
        doctor.contains("backups pode ser lida por outros"),
        "{doctor}"
    );
}

fn notification_daemon_running() -> bool {
    let out = Command::new("dbus-send")
        .args([
            "--session",
            "--print-reply",
            "--dest=org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus.NameHasOwner",
            "string:org.freedesktop.Notifications",
        ])
        .output();
    out.is_ok_and(|o| String::from_utf8_lossy(&o.stdout).contains("boolean true"))
}

#[test]
fn notify_send_shows_a_real_notification() {
    use agsync::notify::Notifier;
    let session =
        std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some();
    if agsync::which::find("notify-send").is_none() || !session || !notification_daemon_running() {
        eprintln!(
            "SKIP notify-send: precisa de notify-send, sessao grafica e um servidor de notificacoes no D-Bus"
        );
        return;
    }
    agsync::notify::notify_send::NotifySend
        .notify("agsync", "teste do agsync: notify-send real")
        .unwrap();
}

/// The directory holding each of `tools`, deduplicated.
fn dirs_of(tools: &[&str]) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = Vec::new();
    for tool in tools {
        let dir = agsync::which::find(tool)
            .unwrap()
            .parent()
            .unwrap()
            .to_path_buf();
        if !dirs.contains(&dir) {
            dirs.push(dir);
        }
    }
    dirs
}

#[test]
fn hook_runs_in_real_interactive_bash_zsh_and_fish() {
    let shells = ["bash", "zsh", "fish"];
    let missing: Vec<&str> = shells
        .into_iter()
        .filter(|s| agsync::which::find(s).is_none())
        .collect();
    if !missing.is_empty() {
        eprintln!("SKIP shells: {missing:?} nao estao no PATH");
        return;
    }
    let mut env = codex_env();
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push("version: 1\nmcps:\n  echo:\n    command: node\n");
    let remote = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &remote, "--profile", "work"]);
    env.ok(&["check"]);
    let head = git(&env.remote, &["rev-parse", "HEAD"]);
    let line = format!(
        "agsync: 1 mudanca pendente ({}); rode `agsync apply`",
        &head[..7]
    );

    let mut dirs = std::env::split_paths(env.path.as_ref().unwrap()).collect::<Vec<_>>();
    dirs.extend(dirs_of(&shells));
    env.path = Some(std::env::join_paths(dirs).unwrap());
    let out = env.ok(&["hook", "install", "--yes"]);
    for shell in shells {
        assert!(out.contains(&format!("{shell}: hook instalado")), "{out}");
    }

    for shell in shells {
        let mut cmd = Command::new(agsync::which::find(shell).unwrap());
        cmd.args(["-i", "-c", "true"]);
        env.isolate(&mut cmd);
        let out = cmd.output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        eprintln!("$ {shell} -i -c true\n{}", text(&out));
        assert!(out.status.success(), "{shell}: {}", text(&out));
        assert_eq!(stdout.matches(&line).count(), 1, "{shell}: {}", text(&out));
    }

    env.ok(&["hook", "remove"]);
    for shell in shells {
        let mut cmd = Command::new(agsync::which::find(shell).unwrap());
        cmd.args(["-i", "-c", "true"]);
        env.isolate(&mut cmd);
        let out = cmd.output().unwrap();
        assert!(!text(&out).contains("agsync:"), "{shell}: {}", text(&out));
    }
}
