//! End to end for `imports`: a second local repo gives MCPs and a skill to the work
//! profile; an import of an inactive profile points at a repo that does not exist.
//!
//! Skipped (with a message) when `claude` is not in PATH.

mod common;

use std::path::PathBuf;

use common::{Env, claude_missing, git, path_with};

fn team_repo(env: &Env) -> (PathBuf, String) {
    let dir = env.root.path().join("team");
    let skill = dir.join("skills").join("team-skill");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nname: team-skill\ndescription: test\n---\nv1\n",
    )
    .unwrap();
    git(&dir, &["init", "-q", "-b", "main"]);
    commit_team(&dir, &team_yaml(""));
    let url = dir.to_string_lossy().replace('\\', "/");
    (dir, url)
}

fn team_yaml(extra: &str) -> String {
    format!(
        "version: 1\nmcps:\n  team-mcp:\n    command: node\n  team-test:\n    command: node\n{extra}skills:\n  - path: skills/team-skill\n"
    )
}

fn commit_team(dir: &std::path::Path, yaml: &str) {
    std::fs::write(dir.join("agsync.yaml"), yaml).unwrap();
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "team"]);
}

#[test]
fn imports_follow_profiles_and_the_imported_branch() {
    if claude_missing("imports_follow_profiles_and_the_imported_branch") {
        return;
    }
    let mut env = Env::new();
    env.path = Some(path_with(&["git", "claude", "sh"], &[]));
    let (team, team_url) = team_repo(&env);
    let missing = env.root.path().join("no-such-repo");
    let missing = missing.to_string_lossy().replace('\\', "/");
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push(&format!(
        r#"version: 1
profiles: [work, personal]
imports:
  mcps:
    - git: "{team_url}"
      profiles: [work]
      excludes: [team-test]
    - git: "{missing}"
      profiles: [personal]
  skills:
    - git: "{team_url}"
      profiles: [work]
"#
    ));
    let url = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &url, "--profile", "work"]);

    let status = env.ok(&["status"]);
    let mcp = format!("mcp team-mcp: adicionar [stdio: node] (de imports.mcps {team_url}@main)");
    assert!(status.contains(&mcp), "{status}");
    let skill = format!("skill team-skill: adicionar (de imports.skills {team_url}@main)");
    assert!(status.contains(&skill), "{status}");
    assert!(!status.contains("team-test"), "{status}");
    env.ok(&["apply", "--yes"]);
    assert!(env.servers().contains_key("team-mcp"));
    assert!(!env.servers().contains_key("team-test"));
    assert!(env.skill("team-skill").join("SKILL.md").is_file());

    env.ok(&["check"]);
    assert!(env.pending().is_none(), "{}", env.check_log());

    commit_team(&team, &team_yaml("  team-new:\n    command: node\n"));
    env.ok(&["check"]);
    let pending = env.pending().expect("pending after a new imported commit");
    assert_eq!(pending["kind"], "changes", "{pending}");
    env.ok(&["apply", "--yes"]);
    assert!(env.servers().contains_key("team-new"));

    std::fs::write(
        env.config.join("machine.toml"),
        format!("repo = '{url}'\nref = 'main'\nprofiles = ['work', 'personal']\n"),
    )
    .unwrap();
    let (ok, text) = env.agsync(&["status"]);
    assert!(!ok, "{text}");
    assert!(
        text.contains(&format!("imports.mcps {missing}@main")),
        "{text}"
    );
}

/// Runs of `git <command>` since the last call, read from a `GIT_TRACE` log.
fn git_runs(trace: &std::path::Path, command: &str) -> usize {
    let text = std::fs::read_to_string(trace).unwrap_or_default();
    std::fs::remove_file(trace).ok();
    let needle = format!("built-in: git {command} ");
    text.lines().filter(|l| l.contains(&needle)).count()
}

#[test]
fn imported_manifest_is_fetched_once_per_commit() {
    let mut env = Env::new();
    env.path = Some(path_with(&["git", "sh"], &[]));
    let (team, team_url) = team_repo(&env);
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push(&format!(
        "version: 1\nimports:\n  mcps:\n    - git: \"{team_url}\"\n"
    ));
    let url = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &url, "--profile", "work"]);
    let trace = env.root.path().join("git-trace.log");
    env.vars
        .push(("GIT_TRACE".into(), trace.to_string_lossy().to_string()));
    let fetches = |env: &Env, ok: bool| {
        let (passed, text) = env.agsync(&["status"]);
        assert_eq!(passed, ok, "{text}");
        (git_runs(&trace, "fetch"), text)
    };

    assert_eq!(fetches(&env, true).0, 0, "both repos cached");
    assert_eq!(fetches(&env, true).0, 0, "both repos cached");

    std::fs::remove_dir_all(team.join("skills")).unwrap();
    commit_team(&team, &team_yaml(""));
    let (count, text) = fetches(&env, false);
    assert_eq!(count, 1, "new commit snapshots the import");
    assert!(text.contains("sem SKILL.md"), "{text}");
    assert_eq!(fetches(&env, false).0, 1, "failed check is not cached");

    commit_team(&team, "version: 1\n");
    assert_eq!(fetches(&env, true).0, 1);
    let cache = env.data.join("imports");
    for entry in std::fs::read_dir(&cache).unwrap() {
        std::fs::write(entry.unwrap().path(), "version: [").unwrap();
    }
    assert_eq!(fetches(&env, true).0, 1, "corrupt entry is a miss");
    assert_eq!(fetches(&env, true).0, 0);
}
