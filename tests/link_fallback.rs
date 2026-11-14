//! Skill links when the OS refuses them, forced through `AGSYNC_TEST_LINK_FAIL`:
//! a refused symlink falls back to a junction on Windows, and a refused junction (or a
//! refused symlink on Linux) to a copy. The state records the mode used.

mod common;

use std::path::Path;

use common::{Env, fake_bin, git, path_with};

fn skill_repo(env: &Env, text: &str) -> String {
    let dir = env.root.path().join("source");
    let skill = dir.join("skills").join("fallback-skill");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        format!("---\nname: fallback-skill\ndescription: test\n---\n{text}\n"),
    )
    .unwrap();
    if !dir.join(".git").exists() {
        git(&dir, &["init", "-q", "-b", "main"]);
    }
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", text]);
    dir.to_string_lossy().replace('\\', "/")
}

fn setup() -> Env {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex"]);
    env.path = Some(path_with(&["git", "sh"], &[bin]));
    let url = skill_repo(&env, "v1");
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push(&format!(
        "version: 1\nskills:\n  - git: \"{url}\"\n    ref: main\n    path: skills/fallback-skill\n"
    ));
    let remote = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &remote, "--profile", "work"]);
    env
}

fn set_failing(env: &mut Env, kinds: &str) {
    let log = env.root.path().join("fake.log");
    env.vars = vec![
        (
            "FAKE_AGENT_LOG".to_string(),
            log.to_string_lossy().to_string(),
        ),
        ("AGSYNC_TEST_LINK_FAIL".to_string(), kinds.to_string()),
    ];
}

fn link(env: &Env) -> std::path::PathBuf {
    env.home
        .join(".agents")
        .join("skills")
        .join("fallback-skill")
}

fn skill_text(env: &Env) -> String {
    std::fs::read_to_string(link(env).join("SKILL.md")).unwrap()
}

fn is_link(path: &Path) -> bool {
    std::fs::symlink_metadata(path).unwrap().is_symlink()
}

#[cfg(windows)]
#[test]
fn refused_symlink_falls_back_to_junction_then_copy() {
    let mut env = setup();
    set_failing(&mut env, "symlink");
    env.ok(&["apply", "--yes"]);
    assert!(
        env.state().contains("\"mode\": \"junction\""),
        "{}",
        env.state()
    );
    assert!(is_link(&link(&env)), "a junction is a reparse point");
    let target = std::fs::read_link(link(&env)).unwrap();
    assert!(
        target.to_string_lossy().contains("skills"),
        "{}",
        target.display()
    );
    assert!(skill_text(&env).contains("v1"));
    assert!(env.ok(&["status"]).contains("nada a mudar"));

    skill_repo(&env, "v2");
    set_failing(&mut env, "symlink,junction");
    env.ok(&["apply", "--yes"]);
    assert!(
        env.state().contains("\"mode\": \"copy\""),
        "{}",
        env.state()
    );
    assert!(!is_link(&link(&env)));
    assert!(skill_text(&env).contains("v2"));
    assert!(env.ok(&["status"]).contains("nada a mudar"));
}

#[cfg(not(windows))]
#[test]
fn refused_symlink_falls_back_to_copy() {
    let mut env = setup();
    set_failing(&mut env, "symlink");
    env.ok(&["apply", "--yes"]);
    assert!(
        env.state().contains("\"mode\": \"copy\""),
        "{}",
        env.state()
    );
    assert!(!is_link(&link(&env)));
    assert!(skill_text(&env).contains("v1"));
    assert!(env.ok(&["status"]).contains("nada a mudar"));
}
