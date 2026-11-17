//! End to end for the new version notice: a local git repo with `vX.Y.Z` tags stands
//! in for the release repo.

mod common;

use std::path::{Path, PathBuf};

use common::{Env, fake_bin, git, path_with};

const CURRENT: &str = env!("CARGO_PKG_VERSION");

fn setup() -> Env {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex"]);
    env.path = Some(path_with(&["git", "sh", "curl"], &[bin]));
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

/// A git repo under the env root with one commit and these tags.
fn releases(env: &Env, name: &str, tags: &[&str]) -> PathBuf {
    let dir = env.root.path().join(name);
    std::fs::create_dir_all(&dir).unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    std::fs::write(dir.join("README"), "x").unwrap();
    git(&dir, &["add", "-A"]);
    git(&dir, &["commit", "-q", "-m", "x"]);
    for tag in tags {
        git(&dir, &["tag", tag]);
    }
    dir
}

fn use_releases(env: &mut Env, dir: &Path) {
    env.vars.retain(|(key, _)| key != "AGSYNC_RELEASES_URL");
    env.vars.push((
        "AGSYNC_RELEASES_URL".to_string(),
        dir.to_string_lossy().to_string(),
    ));
}

fn update_notices(env: &Env) -> Vec<String> {
    env.notifications()
        .into_iter()
        .filter(|n| n.contains("disponivel"))
        .collect()
}

#[test]
fn check_records_a_new_version_and_notifies_once() {
    let mut env = setup();
    let repo = releases(
        &env,
        "releases",
        &["v0.0.1", "v99.0.0", "v99.1.0-rc1", "nightly"],
    );
    use_releases(&mut env, &repo);

    env.ok(&["check"]);
    let saved = std::fs::read_to_string(env.data.join("update.json")).unwrap();
    assert!(saved.contains("\"99.0.0\""), "{saved}");
    let line = format!("agsync 99.0.0 disponivel (atual {CURRENT}): rode `agsync update`");
    assert_eq!(update_notices(&env).len(), 1);
    assert!(update_notices(&env)[0].contains(&line));

    env.ok(&["check"]);
    assert_eq!(update_notices(&env).len(), 1, "same version notified twice");
    assert!(
        env.check_log().contains("(ja avisado)"),
        "{}",
        env.check_log()
    );

    let out = env.ok(&["hook"]);
    assert!(out.contains(&line), "{out}");
    let status = env.ok(&["status"]);
    assert!(status.contains(&line), "{status}");
    let doctor = env.agsync(&["doctor"]).1;
    assert!(
        doctor.contains(&format!("info    agsync {CURRENT}")),
        "{doctor}"
    );

    git(&repo, &["tag", "v99.0.1"]);
    env.ok(&["check"]);
    assert_eq!(update_notices(&env).len(), 2);

    let old = releases(&env, "old-releases", &["v0.0.1"]);
    use_releases(&mut env, &old);
    env.ok(&["check"]);
    assert!(!env.data.join("update.json").exists());
    assert!(!env.ok(&["hook"]).contains("disponivel"));
}

#[test]
fn hook_ignores_a_recorded_version_that_is_not_newer() {
    let env = setup();
    let text = format!("{{\"version\": \"{CURRENT}\"}}");
    std::fs::write(env.data.join("update.json"), text).unwrap();
    assert!(!env.ok(&["hook"]).contains("disponivel"));
}

#[test]
fn unreachable_release_repo_is_logged_and_check_goes_on() {
    let env = setup();
    env.ok(&["check"]);
    assert!(
        env.check_log().contains("consulta falhou"),
        "{}",
        env.check_log()
    );
    assert!(env.pending().is_some());
    assert!(!env.data.join("update.json").exists());
}
