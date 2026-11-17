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
    assert!(status.contains("mcp team-mcp: adicionar"), "{status}");
    assert!(status.contains("skill team-skill: adicionar"), "{status}");
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
