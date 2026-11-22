//! End to end for the Antigravity CLI adapter against a fake `agy`.
//!
//! `examples/fake_agent.rs` stands in for `agy`. MCPs and skills never go through it:
//! agsync edits `mcp_config.json` under the temp home. Plugins come from a local
//! marketplace folder holding a `plugin.json`.

mod common;

use std::path::PathBuf;

use common::{Env, fake_bin, git, path_with};
use jsonc_parser::ParseOptions;
use jsonc_parser::cst::CstRootNode;
use serde_json::{Value, json};

const SECRET: &str = "agy-secret-555";
const REF: &str = "${E2E_AGY_TOKEN}";

const FIXTURE: &str = r#"{
  // user comment that must survive
  "mcpServers": {
    "untouched": { "command": "manual-tool", "args": ["keep"], "disabled": false }
  }
}
"#;

fn config(env: &Env) -> PathBuf {
    gemini_dir(env).join("config").join("mcp_config.json")
}

fn gemini_dir(env: &Env) -> PathBuf {
    env.home.join(".gemini")
}

fn servers(env: &Env) -> serde_json::Map<String, Value> {
    let text = std::fs::read_to_string(config(env)).unwrap();
    let root = CstRootNode::parse(&text, &ParseOptions::default())
        .unwrap()
        .to_serde_value()
        .unwrap();
    root["mcpServers"].as_object().unwrap().clone()
}

fn last_call(env: &Env) -> Vec<String> {
    let text = std::fs::read_to_string(env.root.path().join("fake.log")).unwrap();
    let line = text.lines().last().unwrap();
    let call: Vec<String> = serde_json::from_str(line).unwrap();
    assert_eq!(call[0], "agy");
    call[1..].to_vec()
}

fn yaml(market: &str, with_extras: bool) -> String {
    let extras = if with_extras {
        format!(
            "  remote:\n    url: https://example.com/mcp\n    headers: {{Authorization: \"Bearer {REF}\"}}\nplugins:\n  - name: demo\n    marketplace: {{git: '{market}', ref: main}}\n"
        )
    } else {
        String::new()
    };
    format!(
        "version: 1\nprofiles: [work]\nmcps:\n  tool:\n    command: node\n    args: [server.js]\n    env: {{TOKEN: \"{REF}\"}}\n{extras}skills:\n  - path: skills/hello\n"
    )
}

fn setup() -> (Env, String) {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["agy"]);
    env.path = Some(path_with(&["git", "sh"], &[bin]));
    let log = env.root.path().join("fake.log");
    env.vars = vec![
        ("E2E_AGY_TOKEN".to_string(), SECRET.to_string()),
        (
            "FAKE_AGENT_LOG".to_string(),
            log.to_string_lossy().to_string(),
        ),
    ];
    env.forbidden = vec![SECRET.to_string()];
    std::fs::create_dir_all(gemini_dir(&env).join("config")).unwrap();
    std::fs::write(config(&env), FIXTURE).unwrap();

    let market = env.root.path().join("market");
    std::fs::create_dir_all(&market).unwrap();
    std::fs::write(market.join("plugin.json"), r#"{"name":"demo"}"#).unwrap();
    let market = market.to_string_lossy().replace('\\', "/");

    git(&env.remote, &["init", "-q", "-b", "main"]);
    let skill = env.remote.join("skills").join("hello");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nname: hello\ndescription: t\n---\nhi\n",
    )
    .unwrap();
    env.push(&yaml(&market, true));
    let url = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &url, "--profile", "work"]);
    (env, market)
}

#[test]
fn antigravity_gets_mcps_skills_and_plugins() {
    let (env, market) = setup();
    let status = env.ok(&["status"]);
    assert!(status.contains("antigravity:\n"), "{status}");
    assert!(status.contains("plugin demo: adicionar"), "{status}");

    env.ok(&["apply", "--yes"]);

    let text = std::fs::read_to_string(config(&env)).unwrap();
    assert!(text.contains("// user comment that must survive"), "{text}");
    let s = servers(&env);
    assert_eq!(s["untouched"]["disabled"], json!(false));
    assert_eq!(
        s["tool"],
        json!({"command": "node", "args": ["server.js"], "env": {"TOKEN": SECRET}})
    );
    assert_eq!(s["remote"]["serverUrl"], "https://example.com/mcp");
    assert_eq!(
        s["remote"]["headers"]["Authorization"],
        format!("Bearer {SECRET}")
    );

    let skill = gemini_dir(&env).join("config").join("skills").join("hello");
    assert!(skill.join("SKILL.md").is_file());

    assert_eq!(last_call(&env), ["plugin", "install", market.as_str()]);
    let plugin = gemini_dir(&env).join("config").join("plugins").join("demo");
    assert!(plugin.join(".agsync-source.json").is_file());

    let status = env.ok(&["status"]);
    assert!(status.contains("antigravity:\n  nada a mudar"), "{status}");

    env.push(&yaml(&market, false));
    env.ok(&["apply", "--yes"]);
    assert_eq!(last_call(&env), ["plugin", "uninstall", "demo"]);
    assert!(!plugin.exists());
    let s = servers(&env);
    assert!(!s.contains_key("remote") && s.contains_key("untouched"));
    let text = std::fs::read_to_string(config(&env)).unwrap();
    assert!(text.contains("// user comment that must survive"), "{text}");

    let status = env.ok(&["status"]);
    assert!(status.contains("antigravity:\n  nada a mudar"), "{status}");
}
