//! End to end for `agsync hook`: pending line, background check when the last one is
//! old, and how long the hook takes.

mod common;

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{Env, fake_bin, git, path_with};

fn setup() -> Env {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex"]);
    env.path = Some(path_with(&["git", "sh"], &[bin]));
    let log = env.root.path().join("fake.log");
    env.vars = vec![(
        "FAKE_AGENT_LOG".to_string(),
        log.to_string_lossy().to_string(),
    )];
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push("version: 1\nmcps:\n  echo:\n    command: node\n");
    let url = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &url, "--profile", "work"]);
    env
}

fn wait_for(what: &str, done: impl Fn() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < Duration::from_secs(60), "timeout: {what}");
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn run_millis(env: &Env, arg: &str) -> f64 {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_agsync"));
    cmd.arg(arg).stdout(Stdio::null()).stderr(Stdio::null());
    env.isolate(&mut cmd);
    let start = Instant::now();
    assert!(cmd.status().unwrap().success());
    start.elapsed().as_secs_f64() * 1000.0
}

/// Median of 30 runs of `agsync <arg>`, in milliseconds.
fn median_millis(env: &Env, arg: &str) -> f64 {
    let mut times: Vec<f64> = (0..30).map(|_| run_millis(env, arg)).collect();
    times.sort_by(f64::total_cmp);
    times[times.len() / 2]
}

#[test]
fn hook_prints_pending_and_starts_an_overdue_check() {
    let env = setup();
    assert!(!env.data.join("last-check").exists());

    let out = env.ok(&["hook"]);
    assert!(out.is_empty(), "{out}");
    wait_for("background check", || env.pending().is_some());
    wait_for("check.log", || env.check_log().contains("notificado"));
    assert!(env.data.join("last-check").exists());
    assert_eq!(env.notifications().len(), 1);

    let out = env.ok(&["hook"]);
    let head = git(&env.remote, &["rev-parse", "HEAD"]);
    let line = format!(
        "agsync: 1 mudanca pendente ({}); rode `agsync apply`",
        &head[..7]
    );
    assert_eq!(out.trim(), line);
    let (ok, status) = env.agsync(&["status"]);
    assert!(ok, "{status}");
    assert!(status.contains(&line), "{status}");

    let log_before = env.check_log();
    let baseline = median_millis(&env, "--version");
    let hook = median_millis(&env, "hook");
    eprintln!(
        "hook: mediana {hook:.1} ms; `agsync --version`: {baseline:.1} ms (30 execucoes cada, com pending.json)"
    );
    assert!(
        hook < baseline + 15.0,
        "hook {hook} ms, baseline {baseline} ms"
    );
    std::thread::sleep(Duration::from_millis(500));
    assert_eq!(env.check_log(), log_before, "fresh check started again");

    env.ok(&["apply", "--yes"]);
    let out = env.ok(&["hook"]);
    assert!(out.is_empty(), "{out}");
}

#[test]
fn hook_without_machine_does_nothing() {
    let env = Env::new();
    let out = env.ok(&["hook"]);
    assert!(out.is_empty(), "{out}");
    assert!(!env.data.join("hook-started-check").exists());
}
