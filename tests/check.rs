//! End to end for `agsync check`: temp home, data and remote repo; a fake `codex` is
//! the only agent, and notifications go to a file.

mod common;

use std::fs::OpenOptions;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use common::{Env, fake_bin, git, path_with};

fn yaml(arg: &str) -> String {
    format!("version: 1\nmcps:\n  echo:\n    command: node\n    args: [\"{arg}\"]\n")
}

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
    env.push(&yaml("v1.js"));
    let url = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &url, "--profile", "work"]);
    env
}

#[test]
fn check_records_news_once_and_apply_clears_it() {
    let env = setup();
    let head = git(&env.remote, &["rev-parse", "HEAD"]);

    env.ok(&["check"]);
    let pending = env.pending().expect("pending.json after a new commit");
    assert_eq!(pending["commit"], head.as_str());
    assert_eq!(pending["kind"], "changes");
    assert_eq!(pending["agents"]["codex"]["added"][0], "mcp echo");
    assert_eq!(env.notifications().len(), 1);
    assert!(env.notifications()[0].contains("1 mudanca pendente; rode `agsync apply`"));

    env.ok(&["check"]);
    assert_eq!(env.notifications().len(), 1, "same commit notified twice");
    assert!(
        env.check_log().contains("sem novidade"),
        "{}",
        env.check_log()
    );

    env.push(&yaml("v2.js"));
    env.ok(&["check"]);
    assert_eq!(env.notifications().len(), 2);
    let head = git(&env.remote, &["rev-parse", "HEAD"]);
    assert_eq!(env.pending().unwrap()["commit"], head.as_str());

    env.ok(&["apply", "--yes"]);
    assert!(env.pending().is_none(), "apply left pending.json");
    env.ok(&["check"]);
    assert!(env.pending().is_none());
    assert_eq!(env.notifications().len(), 2);

    env.push("version: 1\nmcps:\n  echo:\n    command: node\n    bogus: 1\n");
    env.ok(&["check"]);
    let bad = env.pending().expect("pending error for invalid yaml");
    assert_eq!(bad["kind"], "invalid");
    assert_eq!(bad["line"], 5);
    assert!(bad["reason"].as_str().unwrap().contains("bogus"), "{bad}");
    assert!(bad.get("agents").is_none());
    assert_eq!(env.notifications().len(), 3);
    assert!(env.notifications()[2].contains("agsync.yaml invalido no commit"));
    env.ok(&["check"]);
    assert_eq!(env.notifications().len(), 3);
}

#[test]
fn unreachable_remote_is_logged_and_exits_zero() {
    let env = setup();
    let gone = env.root.path().join("no-such-repo");
    let text = format!(
        "repo = '{}'\nref = 'main'\nprofiles = ['work']\n",
        gone.display()
    );
    std::fs::write(env.config.join("machine.toml"), text).unwrap();
    env.ok(&["check"]);
    assert!(
        env.check_log().contains("remoto inacessivel"),
        "{}",
        env.check_log()
    );
    assert!(env.pending().is_none());
    assert!(env.notifications().is_empty());
    assert!(!env.data.join("last-check").exists());
}

#[test]
fn busy_lock_makes_check_quiet_and_apply_wait() {
    let env = setup();
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(env.data.join("agsync.lock"))
        .unwrap();
    lock.try_lock().unwrap();

    let (ok, out) = env.agsync(&["check"]);
    assert!(ok, "{out}");
    assert!(out.is_empty(), "{out}");
    assert!(env.check_log().is_empty());
    assert!(env.pending().is_none());

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_agsync"));
    cmd.args(["apply", "--yes"]);
    env.isolate(&mut cmd);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().unwrap();
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(1500) {
        assert!(child.try_wait().unwrap().is_none(), "apply did not wait");
        std::thread::sleep(Duration::from_millis(100));
    }
    assert!(!env.data.join("state.json").exists());
    drop(lock);
    let out = child.wait_with_output().unwrap();
    let text = common::text(&out);
    assert!(out.status.success(), "{text}");
    assert!(text.contains("esperando ele terminar"), "{text}");
    assert!(env.data.join("state.json").exists());
}
