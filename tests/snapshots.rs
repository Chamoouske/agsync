//! End to end for the checkout cache of the config repo: while the remote commit does
//! not change, plans only run `git ls-remote` against it.
//!
//! `PATH` holds a fake `codex`, git and sh, so no real agent runs.

mod common;

use std::path::Path;

use common::{Env, fake_bin, git, path_with};

/// `git <command>` runs recorded in `trace` since the last call; empties the trace.
fn git_runs(trace: &Path, commands: &[&str]) -> usize {
    let text = std::fs::read_to_string(trace).unwrap_or_default();
    std::fs::remove_file(trace).ok();
    let needles: Vec<String> = commands
        .iter()
        .map(|c| format!("built-in: git {c} "))
        .collect();
    text.lines()
        .filter(|l| needles.iter().any(|n| l.contains(n)))
        .count()
}

#[test]
fn config_repo_is_checked_out_once_per_commit() {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex"]);
    env.path = Some(path_with(&["git", "sh"], &[bin]));
    let log = env.root.path().join("fake.log");
    env.vars
        .push(("FAKE_AGENT_LOG".into(), log.to_string_lossy().to_string()));
    git(&env.remote, &["init", "-q", "-b", "main"]);
    let skill = env.remote.join("skills").join("hello");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nname: hello\ndescription: t\n---\nhi\n",
    )
    .unwrap();
    env.push("version: 1\nskills:\n  - path: skills/hello\n");
    let url = env.remote.to_string_lossy().to_string();
    let trace = env.root.path().join("git-trace.log");
    env.vars
        .push(("GIT_TRACE".into(), trace.to_string_lossy().to_string()));
    let downloads = |env: &Env, args: &[&str]| {
        env.ok(args);
        git_runs(&trace, &["fetch", "clone"])
    };
    let lookups = |env: &Env, args: &[&str]| {
        env.ok(args);
        git_runs(&trace, &["ls-remote"])
    };

    let init = ["init", &url, "--profile", "work"];
    assert_eq!(downloads(&env, &init), 1, "first plan checks out");
    assert_eq!(downloads(&env, &["status"]), 0, "same commit: cache");
    assert_eq!(lookups(&env, &["status"]), 1, "remote is still asked");
    assert_eq!(downloads(&env, &["apply", "--yes"]), 0);
    let applied = env.home.join(".agents").join("skills").join("hello");
    assert!(applied.join("SKILL.md").is_file(), "skill from cached tree");
    assert_eq!(downloads(&env, &["check"]), 0);

    env.push("version: 1\nmcps:\n  a:\n    command: node\nskills:\n  - path: skills/hello\n");
    assert_eq!(downloads(&env, &["check"]), 1, "new commit checks out");
    assert_eq!(downloads(&env, &["status"]), 0);
    assert_eq!(downloads(&env, &["diff"]), 0);
}
