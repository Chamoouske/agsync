//! The Linux scheduler picks systemd, else cron, else nothing, as `PATH` allows.
//!
//! Alone in its test binary because it changes the process `PATH`. The schedule has a
//! test name and runs a probe script, as in `linux_schedule.rs`.

#![cfg(target_os = "linux")]

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use agsync::schedule::cron::Cron;
use agsync::schedule::systemd::Systemd;
use agsync::schedule::{self, Scheduler};
use common::Env;

struct Cleanup {
    name: String,
    path: std::ffi::OsString,
    table: Option<Vec<u8>>,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        set_path(&self.path);
        let _ = Systemd::runtime(&self.name).unwrap().remove();
        let _ = Cron::new(&self.name).remove();
        restore_crontab(self.table.as_deref());
    }
}

fn read_crontab() -> Option<Vec<u8>> {
    let out = Command::new("crontab").arg("-l").output().ok()?;
    out.status.success().then_some(out.stdout)
}

fn restore_crontab(table: Option<&[u8]>) {
    let Some(table) = table else {
        let _ = Command::new("crontab").arg("-r").output();
        return;
    };
    let mut child = Command::new("crontab")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    std::io::Write::write_all(&mut child.stdin.take().unwrap(), table).unwrap();
    assert!(child.wait().unwrap().success());
}

fn set_path(path: &std::ffi::OsStr) {
    unsafe { std::env::set_var("PATH", path) };
}

fn probe(dir: &Path) -> PathBuf {
    let script = dir.join("probe");
    std::fs::write(&script, "#!/bin/sh\nexit 0\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    script
}

fn timer_enabled(systemctl: &Path, name: &str) -> bool {
    Command::new(systemctl)
        .args(["--user", "is-enabled", &format!("{name}.timer")])
        .output()
        .is_ok_and(|o| o.status.success())
}

fn cron_has(crontab: &Path, name: &str) -> bool {
    let out = Command::new(crontab).arg("-l").output().unwrap();
    String::from_utf8_lossy(&out.stdout).contains(&format!("# {name}"))
}

#[test]
fn systemd_then_cron_then_nothing() {
    let systemctl = agsync::which::find("systemctl");
    let crontab = agsync::which::find("crontab");
    let (Some(systemctl), Some(crontab)) = (systemctl, crontab) else {
        eprintln!("SKIP chain: precisa de systemctl e crontab no PATH");
        return;
    };
    let env = Env::new();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let name = format!("agsync-test-chain-{}-{nanos}", std::process::id());
    let cleanup = Cleanup {
        name: name.clone(),
        path: std::env::var_os("PATH").unwrap(),
        table: read_crontab(),
    };
    unsafe { std::env::set_var(schedule::TEST_NAME_VAR, &name) };
    let exe = probe(env.root.path());

    schedule::for_this_machine().install(&exe, 15).unwrap();
    assert!(timer_enabled(&systemctl, &name), "systemd first");
    assert!(!cron_has(&crontab, &name));
    assert!(schedule::for_this_machine().remove().unwrap());
    assert!(!timer_enabled(&systemctl, &name));

    let cron_only = env.root.path().join("cron-only");
    std::fs::create_dir_all(&cron_only).unwrap();
    std::os::unix::fs::symlink(&crontab, cron_only.join("crontab")).unwrap();
    set_path(cron_only.as_os_str());
    schedule::for_this_machine().install(&exe, 15).unwrap();
    assert!(cron_has(&crontab, &name), "cron without systemctl");
    assert!(!timer_enabled(&systemctl, &name));
    assert!(schedule::for_this_machine().remove().unwrap());
    assert!(!cron_has(&crontab, &name));

    let empty = env.root.path().join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    set_path(empty.as_os_str());
    let error = schedule::for_this_machine().install(&exe, 15).unwrap_err();
    assert!(
        format!("{error:#}").contains("o `agsync hook` dispara o check"),
        "{error:#}"
    );
    let table = cleanup.table.clone();
    drop(cleanup);
    assert!(!timer_enabled(&systemctl, &name));
    assert_eq!(read_crontab(), table);
}
