//! Real Linux schedulers under a test name (`agsync-test-<random>`), so the real
//! `agsync-check` is never created.
//!
//! The scheduled program is a probe script that only writes a file in the test's temp
//! dir: a timer that fires runs nothing of the user. The systemd timer lives in the
//! runtime dir, enabled with `--runtime`, so nothing lands in the home. The cron test
//! keeps a line of someone else in the crontab. Guards remove the units and restore the
//! crontab on drop, also when an assertion failed.

#![cfg(target_os = "linux")]

mod common;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use agsync::schedule::cron::Cron;
use agsync::schedule::systemd::Systemd;
use agsync::schedule::{Scheduler, Status};
use common::{Env, text};

fn unique_name(kind: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("agsync-test-{kind}-{}-{nanos}", std::process::id())
}

/// Executable script that writes `<dir>/ran` with its arguments.
fn probe(dir: &Path) -> PathBuf {
    let script = dir.join("probe dir").join("probe");
    std::fs::create_dir_all(script.parent().unwrap()).unwrap();
    let ran = dir.join("ran");
    std::fs::write(
        &script,
        format!("#!/bin/sh\necho \"$@\" > '{}'\n", ran.display()),
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script
}

fn systemctl(args: &[&str]) -> (bool, String) {
    let out = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .unwrap();
    (out.status.success(), text(&out).trim().to_string())
}

fn crontab_list() -> Option<String> {
    let out = Command::new("crontab").arg("-l").output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).to_string())
}

fn crontab_write(table: Option<&str>) {
    let Some(table) = table else {
        let _ = Command::new("crontab").arg("-r").output();
        return;
    };
    let mut child = Command::new("crontab")
        .arg("-")
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(table.as_bytes())
        .unwrap();
    assert!(child.wait().unwrap().success());
}

/// Removes the test timer and its units.
struct SystemdGuard(String);

impl Drop for SystemdGuard {
    fn drop(&mut self) {
        let _ = Systemd::runtime(&self.0).unwrap().remove();
    }
}

/// Puts back the crontab found when the test started.
struct CrontabGuard(Option<String>);

impl Drop for CrontabGuard {
    fn drop(&mut self) {
        crontab_write(self.0.as_deref());
    }
}

fn real_check_absent() {
    let (found, _) = systemctl(&["cat", "agsync-check.timer"]);
    assert!(!found, "the real agsync-check.timer exists");
    let table = crontab_list().unwrap_or_default();
    assert!(!table.contains("# agsync-check"), "{table}");
}

fn wait_for(what: &str, done: impl Fn() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < Duration::from_secs(30), "timeout: {what}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

#[test]
fn systemd_user_timer_is_installed_fires_and_is_removed() {
    if !systemctl(&["show-environment"]).0 {
        eprintln!("SKIP systemd: `systemctl --user` nao responde");
        return;
    }
    real_check_absent();
    let env = Env::new();
    let name = unique_name("systemd");
    let guard = SystemdGuard(name.clone());
    let systemd = Systemd::runtime(&name).unwrap();
    let timer = format!("{name}.timer");
    let exe = probe(env.root.path());
    assert_eq!(systemd.status(&exe).unwrap(), Status::Missing);

    let notes = systemd.install(&exe, 15).unwrap();
    assert!(notes[0].contains("na unit"), "{notes:?}");
    assert_eq!(systemctl(&["is-enabled", &timer]).1, "enabled-runtime");
    assert_eq!(systemctl(&["is-active", &timer]).1, "active");
    let (_, shown) = systemctl(&["show", "-p", "TimersMonotonic", &timer]);
    assert!(shown.contains("OnUnitActiveUSec=15min"), "{shown}");
    let ran = env.root.path().join("ran");
    wait_for("timer runs the probe", || ran.is_file());
    assert_eq!(std::fs::read_to_string(&ran).unwrap(), "check\n");

    assert_eq!(systemd.status(&exe).unwrap(), Status::Current);
    let other = systemd.status(Path::new("/old/agsync")).unwrap();
    assert!(matches!(other, Status::Other(_)), "{other:?}");

    assert!(systemd.remove().unwrap());
    assert!(!systemctl(&["is-enabled", &timer]).0);
    assert!(!systemd.dir.join(&timer).exists());
    assert!(!systemd.remove().unwrap());
    assert_eq!(systemd.status(&exe).unwrap(), Status::Missing);
    drop(guard);
    real_check_absent();
}

#[test]
fn cron_line_is_added_and_removed_next_to_other_lines() {
    if agsync::which::find("crontab").is_none() {
        eprintln!("SKIP cron: `crontab` nao esta no PATH");
        return;
    }
    let env = Env::new();
    let guard = CrontabGuard(crontab_list());
    let name = unique_name("cron");
    let cron = Cron::new(&name);
    let exe = probe(env.root.path());
    let foreign = format!("0 3 * * * true # {}", unique_name("foreign"));
    let mut start = guard.0.clone().unwrap_or_default();
    start.push_str(&foreign);
    start.push('\n');
    crontab_write(Some(&start));

    let notes = cron.install(&exe, 15).unwrap();
    assert!(notes[0].contains("na linha do crontab"), "{notes:?}");
    let table = crontab_list().unwrap();
    let line = table
        .lines()
        .find(|l| l.ends_with(&format!("# {name}")))
        .unwrap_or_else(|| panic!("no agsync line:\n{table}"));
    assert!(line.starts_with("*/15 * * * * env 'PATH="), "{line}");
    assert!(
        line.contains(&format!("'{}' check", exe.display())),
        "{line}"
    );
    assert!(table.starts_with(&start), "{table}");
    assert_eq!(cron.status(&exe).unwrap(), Status::Current);

    assert!(cron.remove().unwrap());
    assert_eq!(crontab_list().unwrap(), start);
    assert!(!cron.remove().unwrap());
    assert_eq!(cron.status(&exe).unwrap(), Status::Missing);

    crontab_write(guard.0.as_deref());
    assert_eq!(crontab_list(), guard.0);
    drop(guard);
    real_check_absent();
}

#[test]
fn without_systemctl_and_crontab_install_points_at_the_hook() {
    let env = Env::new();
    let bin = env.root.path().join("emptybin");
    std::fs::create_dir_all(&bin).unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_agsync"));
    cmd.args(["schedule", "install"]);
    env.isolate(&mut cmd);
    cmd.env_remove("AGSYNC_TEST_SCHEDULE_FILE")
        .env("AGSYNC_TEST_SCHEDULE_NAME", unique_name("none"))
        .env("PATH", &bin);
    let out = cmd.output().unwrap();
    let out_text = text(&out);
    assert!(!out.status.success(), "{out_text}");
    assert!(
        out_text.contains(
            "sem `systemctl --user` funcional nem `crontab`: o `agsync hook` dispara o check"
        ),
        "{out_text}"
    );
}
