//! Home, data dir, skill cache, source checkouts and agent config all under a path with
//! spaces, with the real `claude`. Skipped (with a message) when `claude` is not in
//! PATH.

mod common;

use common::{Env, claude_missing, git, path_with};

#[test]
fn paths_with_spaces_work_end_to_end() {
    if claude_missing("paths_with_spaces_work_end_to_end") {
        return;
    }
    let mut env = Env::named("agsync e2e with spaces");
    assert!(env.home.to_string_lossy().contains(' '));
    env.path = Some(path_with(&["git", "claude", "sh"], &[]));

    let source = env.root.path().join("source repo");
    let skill = source.join("skills").join("spaced-skill");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nname: spaced-skill\ndescription: test\n---\nv1\n",
    )
    .unwrap();
    git(&source, &["init", "-q", "-b", "main"]);
    git(&source, &["add", "-A"]);
    git(&source, &["commit", "-q", "-m", "one"]);
    let url = source.to_string_lossy().replace('\\', "/");
    let yaml = format!(
        r#"version: 1
mcps:
  built:
    command: node
    args: ["{{dir}}/out.txt"]
    env: {{TOKEN: "${{E2E_SPACES_SECRET}}"}}
    source:
      git: "{url}"
      ref: main
      build: ["echo built> out.txt"]
skills:
  - git: "{url}"
    ref: main
    path: skills/spaced-skill
"#
    );
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push(&yaml);
    let remote = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &remote, "--profile", "work"]);
    let (ok, out) = env.agsync_with_input(
        &["secret", "set", "E2E_SPACES_SECRET"],
        Some("spaced-secret\n"),
    );
    assert!(ok, "{out}");

    env.ok(&["apply", "--yes"]);
    let arg = env.args_of("built")[0].as_str().unwrap().to_string();
    assert!(arg.contains(' '), "{arg}");
    let built = std::fs::read_to_string(&arg).unwrap();
    assert_eq!(built.trim(), "built");
    let skill = env.skill("spaced-skill");
    assert!(skill.join("SKILL.md").is_file(), "{}", skill.display());
    assert!(env.ok(&["status"]).contains("nada a mudar"));

    let doctor = env.agsync(&["doctor"]).1;
    assert!(!doctor.contains("PROBLEMA"), "{doctor}");
    env.ok(&["check"]);
    assert!(
        env.check_log().contains("sem novidade"),
        "{}",
        env.check_log()
    );
}
