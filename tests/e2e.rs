//! End to end: local git repo as remote, real `claude` CLI, every dir temporary.
//!
//! Skipped (with a message) when `claude` is not in PATH.

mod common;

use common::{Env, claude_missing, count, git, path_with};

const SECRET: &str = "s3cr3t-token-value";
const ENV_SECRET: &str = "claude-env-secret-555";
const FILE_SECRET: &str = "claude-file-secret-666";

fn yaml(echo_arg: &str, extra_mcps: &str, skills: &str) -> String {
    let skills = if skills.is_empty() {
        " []\n".to_string()
    } else {
        format!("\n{skills}")
    };
    format!(
        r#"version: 1
profiles: [work, personal]
mcps:
  echo:
    command: node
    args: ["{echo_arg}"]
    env: {{TOKEN: "{SECRET}"}}
  home:
    profiles: [personal]
    command: node
{extra_mcps}skills:{skills}"#
    )
}

const HELLO: &str = "  - path: skills/hello\n";

#[test]
fn end_to_end_with_real_claude() {
    if claude_missing("end_to_end_with_real_claude") {
        return;
    }
    let mut env = Env::new();
    env.path = Some(path_with(&["git", "claude", "sh"], &[]));
    env.forbidden = vec![
        SECRET.to_string(),
        ENV_SECRET.to_string(),
        FILE_SECRET.to_string(),
    ];
    env.vars = vec![("E2E_CLAUDE_ENV".to_string(), ENV_SECRET.to_string())];
    git(&env.remote, &["init", "-q", "-b", "main"]);
    let skill_dir = env.remote.join("skills").join("hello");
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        "---\nname: hello\ndescription: test\n---\nhi\n",
    )
    .unwrap();
    env.push(&yaml("v1.js", "", HELLO));

    let untouched = r#"{"type":"stdio","command":"manual-tool","args":["keep"]}"#;
    env.claude(&["mcp", "add-json", "--scope", "user", "untouched", untouched]);
    assert!(env.servers().contains_key("untouched"));

    let url = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &url, "--profile", "work"]);
    assert!(env.config.join("machine.toml").is_file());
    let (ok, out) = env.agsync_with_input(
        &["secret", "set", "E2E_CLAUDE_FILE"],
        Some(&format!("{FILE_SECRET}\n")),
    );
    assert!(ok, "{out}");

    let status = env.ok(&["status"]);
    let head = git(&env.remote, &["rev-parse", "HEAD"]);
    assert!(status.contains("commit aplicado: nenhum"), "{status}");
    assert!(
        status.contains(&format!("commit remoto:   {head}")),
        "{status}"
    );
    assert!(status.contains("mcp echo: adicionar"), "{status}");
    assert!(status.contains("env: TOKEN"), "{status}");
    assert!(status.contains("skill hello: adicionar"), "{status}");
    assert!(
        status.contains("codex: codex nao encontrado no PATH")
            && status.contains("hermes: hermes nao encontrado no PATH"),
        "{status}"
    );
    assert!(!status.contains("home"), "{status}");
    assert!(!status.contains("untouched"), "{status}");

    let diff = env.ok(&["diff"]);
    assert!(diff.contains("mcp echo: adicionar"), "{diff}");

    let (ok, text) = env.agsync(&["apply"]);
    assert!(!ok, "apply without tty and --yes must refuse");
    assert!(text.contains("sem TTY"), "{text}");
    assert!(!env.servers().contains_key("echo"));

    let applied = env.ok(&["apply", "--yes"]);
    assert!(applied.contains("reiniciar"), "{applied}");
    assert_eq!(env.args_of("echo"), serde_json::json!(["v1.js"]));
    assert_eq!(env.servers()["echo"]["env"]["TOKEN"], SECRET);
    assert!(env.skill("hello").join("SKILL.md").is_file());
    let state = env.state();
    assert!(state.contains(&head), "{state}");
    assert!(!state.contains(SECRET), "{state}");
    assert!(!state.contains("TOKEN"), "{state}");
    assert!(env.ok(&["status"]).contains("nada a mudar"));

    let two = "  two:\n    url: https://example.com/mcp\n    headers: {Authorization: \"Bearer ${E2E_CLAUDE_ENV}\", X-Key: \"${E2E_CLAUDE_FILE}\", X-Team: t1}\n";
    env.push(&yaml("v2.js", two, HELLO));
    let status = env.ok(&["status"]);
    assert!(status.contains("mcp echo: atualizar"), "{status}");
    assert!(status.contains("mcp two: adicionar"), "{status}");
    env.ok(&["apply", "--yes"]);
    assert_eq!(env.args_of("echo"), serde_json::json!(["v2.js"]));
    let remote = &env.servers()["two"];
    assert_eq!(remote["type"], "http");
    assert_eq!(remote["url"], "https://example.com/mcp");
    let headers = &remote["headers"];
    assert_eq!(headers["Authorization"], "Bearer ${E2E_CLAUDE_ENV}");
    assert_eq!(headers["X-Key"], FILE_SECRET);
    assert_eq!(headers["X-Team"], "t1");

    env.push(&yaml("v2.js", "", ""));
    let diff = env.ok(&["diff"]);
    assert!(diff.contains("mcp two: remover"), "{diff}");
    assert!(diff.contains("skill hello: remover"), "{diff}");
    env.ok(&["apply", "--yes"]);
    assert!(!env.servers().contains_key("two"));
    assert!(std::fs::symlink_metadata(env.skill("hello")).is_err());
    assert!(!env.state().contains("\"two\""));

    let hacked = r#"{"type":"stdio","command":"node","args":["hacked.js"]}"#;
    env.claude(&["mcp", "remove", "--scope", "user", "echo"]);
    env.claude(&["mcp", "add-json", "--scope", "user", "echo", hacked]);
    let status = env.ok(&["status"]);
    assert!(status.contains("mcp echo: desvio manual"), "{status}");
    let backups_before = count(&env.data.join("backups"));
    env.ok(&["apply", "--yes"]);
    assert_eq!(env.args_of("echo"), serde_json::json!(["v2.js"]));
    assert!(count(&env.data.join("backups")) > backups_before);
    assert!(agsync::secret::file::is_private(&env.data.join("backups")).unwrap());

    let manual = r#"{"type":"stdio","command":"node","args":["manual.js"]}"#;
    env.claude(&["mcp", "add-json", "--scope", "user", "manual", manual]);
    let repo_manual = "  manual:\n    command: node\n    args: [\"repo.js\"]\n";
    env.push(&yaml("v2.js", repo_manual, HELLO));
    let status = env.ok(&["status"]);
    assert!(status.contains("mcp manual: conflito"), "{status}");
    env.ok(&["apply", "--yes"]);
    assert_eq!(env.args_of("manual"), serde_json::json!(["manual.js"]));
    assert!(!env.state().contains("\"manual\""));
    assert!(env.skill("hello").join("SKILL.md").is_file());

    let adopted = env.ok(&["apply", "--yes", "--adopt"]);
    assert!(adopted.contains("assumir"), "{adopted}");
    assert_eq!(env.args_of("manual"), serde_json::json!(["repo.js"]));
    assert!(env.state().contains("\"manual\""));
    assert!(env.ok(&["status"]).contains("nada a mudar"));

    assert_eq!(
        env.servers()["untouched"]["args"],
        serde_json::json!(["keep"])
    );
    let leftovers: Vec<_> = std::fs::read_dir(&env.tmp)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with("agsync-snapshot")
        })
        .collect();
    assert!(leftovers.is_empty(), "snapshot not deleted: {leftovers:?}");
}
