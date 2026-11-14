//! `agsync rollback` against the fake Codex: the previous commit is planned and applied
//! like any other, a source checkout already cleaned from the cache is fetched again,
//! and `check` reports the remote head as pending afterwards.

mod common;

use common::{Env, fake_bin, git, path_with};
use serde_json::Value;

fn skill_commit(env: &Env, text: &str) -> String {
    let dir = env.root.path().join("source");
    let skill = dir.join("skills").join("rolled");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        format!("---\nname: rolled\ndescription: test\n---\n{text}\n"),
    )
    .unwrap();
    if !dir.join(".git").exists() {
        git(&dir, &["init", "-q", "-b", "main"]);
    }
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", text]);
    git(&dir, &["rev-parse", "HEAD"])
}

fn yaml(env: &Env, arg: &str, skill_sha: &str) -> String {
    let url = env
        .root
        .path()
        .join("source")
        .to_string_lossy()
        .replace('\\', "/");
    format!(
        "version: 1\nmcps:\n  plain:\n    command: node\n    args: [\"{arg}\"]\nskills:\n  - git: \"{url}\"\n    ref: \"{skill_sha}\"\n    path: skills/rolled\n"
    )
}

fn codex_args(env: &Env) -> toml::Value {
    let text = std::fs::read_to_string(env.codex_dir.join("config.toml")).unwrap();
    let root: toml::Table = toml::from_str(&text).unwrap();
    root["mcp_servers"]["plain"]["args"].clone()
}

fn skill_text(env: &Env) -> String {
    let link = env.home.join(".agents").join("skills").join("rolled");
    std::fs::read_to_string(link.join("SKILL.md")).unwrap()
}

fn state(env: &Env) -> Value {
    serde_json::from_str(&env.state()).unwrap()
}

fn checkouts(env: &Env) -> Vec<String> {
    let dir = env.data.join("sources").join("rolled");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect()
}

fn args(words: &[&str]) -> toml::Value {
    toml::Value::Array(words.iter().map(|w| toml::Value::from(*w)).collect())
}

#[test]
fn rollback_applies_the_previous_commit_through_the_normal_plan() {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex"]);
    env.path = Some(path_with(&["git", "sh"], &[bin]));
    let log = env.root.path().join("fake.log");
    env.vars = vec![(
        "FAKE_AGENT_LOG".to_string(),
        log.to_string_lossy().to_string(),
    )];
    let skill_v1 = skill_commit(&env, "skill v1");
    let skill_v2 = skill_commit(&env, "skill v2");
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push(&yaml(&env, "v1.js", &skill_v1));
    let first = git(&env.remote, &["rev-parse", "HEAD"]);
    let url = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &url, "--profile", "work"]);
    env.ok(&["apply", "--yes"]);
    assert_eq!(codex_args(&env), args(&["v1.js"]));
    assert_eq!(checkouts(&env), std::slice::from_ref(&skill_v1));

    let (ok, out) = env.agsync(&["rollback", "--yes"]);
    assert!(!ok, "{out}");
    assert!(out.contains("sem commit anterior"), "{out}");

    env.push(&yaml(&env, "v2.js", &skill_v2));
    let second = git(&env.remote, &["rev-parse", "HEAD"]);
    env.ok(&["apply", "--yes"]);
    assert_eq!(codex_args(&env), args(&["v2.js"]));
    assert!(skill_text(&env).contains("skill v2"));
    assert_eq!(state(&env)["previous"], Value::from(first.clone()));
    assert_eq!(checkouts(&env), std::slice::from_ref(&skill_v2));
    env.ok(&["check"]);
    assert!(env.pending().is_none());

    let (ok, out) = env.agsync(&["rollback"]);
    assert!(!ok, "{out}");
    assert!(out.contains("sem TTY"), "{out}");
    assert_eq!(codex_args(&env), args(&["v2.js"]));

    let out = env.ok(&["rollback", "--yes"]);
    assert!(
        out.contains(&format!("rollback: {second} -> {first}")),
        "{out}"
    );
    assert!(out.contains("mcp plain: atualizar"), "{out}");
    assert!(out.contains("skill rolled: atualizar"), "{out}");
    assert!(out.contains("`agsync apply`"), "{out}");
    assert_eq!(codex_args(&env), args(&["v1.js"]));
    assert!(skill_text(&env).contains("skill v1"));
    assert_eq!(state(&env)["commit"], Value::from(first.clone()));
    assert_eq!(state(&env)["previous"], Value::from(second.clone()));

    env.ok(&["check"]);
    let pending = env.pending().expect("remote head pending after rollback");
    assert_eq!(pending["commit"], Value::from(second.clone()));
    assert!(env.ok(&["status"]).contains("mcp plain: atualizar"));

    env.ok(&["apply", "--yes"]);
    assert_eq!(codex_args(&env), args(&["v2.js"]));
    assert!(skill_text(&env).contains("skill v2"));
    assert_eq!(state(&env)["commit"], Value::from(second));
    assert!(env.pending().is_none());
}
