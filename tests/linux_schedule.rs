//! Real Linux schedulers under a test name (`agsync-test-<random>`), so the real
//! `agsync-check` is never created.
//!
//! The scheduled program is a probe script that only writes a file in the test's temp
//! dir: a timer that fires runs nothing of the user. The systemd test starts the
//! service itself, since the first timer run waits for `OnBootSec` after a fresh boot.
//! The systemd timer lives in the runtime dir, enabled with `--runtime`, so nothing
//! lands in the home. The cron test
//! keeps a line of someone else in the crontab. Guards remove the units and restore the
//! crontab on drop, also when an assertion failed.

#![cfg(target_os = "linux")]

mod common;

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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

/// Executable script that writes `<dir>/ran` with its arguments, then its `PATH`.
fn probe(dir: &Path) -> PathBuf {
    let script = dir.join("probe dir").join("probe");
    std::fs::create_dir_all(script.parent().unwrap()).unwrap();
    let ran = dir.join("ran");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\necho \"$@\" > '{0}'\necho \"$PATH\" >> '{0}'\n",
            ran.display()
        ),
    )
    .unwrap();
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

#[test]
fn systemd_user_timer_is_installed_runs_and_is_removed() {
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
    let (_, next) = systemctl(&["show", "-p", "NextElapseUSecMonotonic", &timer]);
    assert!(!next.ends_with('='), "timer not scheduled: {next}");
    let (started, out) = systemctl(&["start", &format!("{name}.service")]);
    assert!(started, "{out}");
    let ran = std::fs::read_to_string(env.root.path().join("ran")).unwrap();
    assert!(ran.starts_with("check\n"), "{ran}");
    let unit_path = ran.lines().nth(1).unwrap();
    let dirs: Vec<PathBuf> = std::env::split_paths(unit_path).collect();
    let host = agsync::host::current();
    for dir in &dirs {
        assert!(
            !host.foreign(dir),
            "Windows dir in the unit PATH: {unit_path}"
        );
        assert_eq!(dirs.iter().filter(|d| *d == dir).count(), 1, "{unit_path}");
    }

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
    let cron = Cron::new(&name, &env.data);
    let exe = probe(env.root.path());
    let foreign = format!("0 3 * * * true # {}", unique_name("foreign"));
    let mut start = guard.0.clone().unwrap_or_default();
    start.push_str(&foreign);
    start.push('\n');
    crontab_write(Some(&start));

    let notes = cron.install(&exe, 15).unwrap();
    assert!(notes[0].contains("no script do cron"), "{notes:?}");
    let table = crontab_list().unwrap();
    let line = table
        .lines()
        .find(|l| l.ends_with(&format!("# {name}")))
        .unwrap_or_else(|| panic!("no agsync line:\n{table}"));
    let script = env.data.join(format!("{name}.sh"));
    assert_eq!(
        line,
        format!(
            "*/15 * * * * '{}' >/dev/null 2>&1 # {name}",
            script.display()
        )
    );
    assert!(table.starts_with(&start), "{table}");
    let mode = std::fs::metadata(&script).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o700);
    assert_eq!(cron.status(&exe).unwrap(), Status::Current);
    let other = cron.status(Path::new("/old/agsync")).unwrap();
    assert!(matches!(other, Status::Other(_)), "{other:?}");

    let ran = Command::new(&script).env_clear().output().unwrap();
    assert!(ran.status.success(), "{}", text(&ran));
    let probe_out = std::fs::read_to_string(env.root.path().join("ran")).unwrap();
    let mut lines = probe_out.lines();
    assert_eq!(lines.next(), Some("check"));
    let host = agsync::host::current();
    let script_path = lines.next().unwrap();
    for dir in std::env::split_paths(script_path) {
        assert!(
            !host.foreign(&dir),
            "Windows dir in the script PATH: {script_path}"
        );
    }

    assert!(cron.remove().unwrap());
    assert_eq!(crontab_list().unwrap(), start);
    assert!(!script.exists());
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
