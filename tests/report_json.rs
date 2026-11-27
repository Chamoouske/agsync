//! `status_view` and `diff_view` run in process against a fake `codex` and serialize
//! to JSON. One test per binary: it sets the process environment of the temp setup.

mod common;

use std::process::Command;

use common::{Env, fake_bin, git, path_with};
use serde_json::json;

const CODEX_FIXTURE: &str = "[mcp_servers.manual]\ncommand = \"manual-tool\"\n";

const YAML: &str = r#"version: 1
profiles: [work]
mcps:
  plain:
    command: node
    args: ["v1.js"]
    env: {MODE: fast}
  manual:
    command: node
"#;

fn setup() -> Env {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex"]);
    env.path = Some(path_with(&["git", "sh"], &[bin]));
    let log = env.root.path().join("fake.log");
    env.vars
        .push(("FAKE_AGENT_LOG".into(), log.to_string_lossy().to_string()));
    std::fs::write(env.codex_dir.join("config.toml"), CODEX_FIXTURE).unwrap();
    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push(YAML);
    let remote = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &remote, "--profile", "work"]);
    env
}

/// Copies the isolated environment agsync runs get into this process.
fn enter(env: &Env) {
    let mut cmd = Command::new("agsync");
    env.isolate(&mut cmd);
    for (key, value) in cmd.get_envs() {
        unsafe {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

#[test]
fn status_and_diff_serialize_to_json() {
    let env = setup();
    enter(&env);
    let status = serde_json::to_value(agsync_core::report::status_view().unwrap()).unwrap();
    let head = git(&env.remote, &["rev-parse", "HEAD"]);
    assert_eq!(status["git_ref"], "main");
    assert_eq!(status["profiles"], json!(["work"]));
    assert_eq!(status["applied"], json!(null));
    assert_eq!(status["remote"], head);
    let codex = json!({
        "agent": "codex",
        "changes": [
            {"kind": "mcp", "name": "manual", "action": "conflict", "detail": null, "origin": null},
            {"kind": "mcp", "name": "plain", "action": "add",
             "detail": "stdio: node v1.js; env: MODE", "origin": null},
        ],
    });
    assert_eq!(status["agents"], json!([codex]));
    assert_eq!(status["builds"], json!([]));
    let skipped = status["skipped"].as_array().unwrap();
    assert!(skipped.contains(&json!("claude: claude nao encontrado no PATH")));

    let diff = serde_json::to_value(agsync_core::report::diff_view().unwrap()).unwrap();
    assert_eq!(diff["agents"], json!([codex]));
    assert!(diff.get("skipped").is_none(), "{diff}");
    assert!(diff.get("remote").is_none(), "{diff}");
}
