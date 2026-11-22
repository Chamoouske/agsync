//! End to end for the Gemini CLI and Antigravity adapters against fake CLIs.
//!
//! `examples/fake_agent.rs` stands in for `gemini` and `agy`. MCPs and skills never go
//! through them: agsync edits `settings.json` and `mcp_config.json` under the temp home.
//! Plugins come from a local marketplace folder that is both a Gemini extension and an
//! Antigravity plugin.

mod common;

use std::path::PathBuf;

use common::{Env, fake_bin, git, path_with};
use jsonc_parser::ParseOptions;
use jsonc_parser::cst::CstRootNode;
use serde_json::{Value, json};

const SECRET: &str = "gemini-agy-secret-555";
const REF: &str = "${E2E_GA_TOKEN}";

const GEMINI_FIXTURE: &str = r#"{
  // user comment that must survive
  "theme": "Default",
  "mcpServers": {
    "untouched": { "command": "manual-tool", "args": ["keep"], "trust": true }
  }
}
"#;

fn gemini_settings(env: &Env) -> PathBuf {
    env.home.join(".gemini").join("settings.json")
}

fn agy_config(env: &Env) -> PathBuf {
    env.home
        .join(".gemini")
        .join("config")
        .join("mcp_config.json")
}

fn servers(file: &PathBuf) -> serde_json::Map<String, Value> {
    let text = std::fs::read_to_string(file).unwrap();
    let root = CstRootNode::parse(&text, &ParseOptions::default())
        .unwrap()
        .to_serde_value()
        .unwrap();
    root["mcpServers"].as_object().unwrap().clone()
}

fn calls(env: &Env, agent: &str) -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(env.root.path().join("fake.log")).unwrap_or_default();
    text.lines()
        .map(|line| serde_json::from_str::<Vec<String>>(line).unwrap())
        .filter(|call| call[0] == agent)
        .map(|call| call[1..].to_vec())
        .collect()
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
    let bin = fake_bin(&env, &["gemini", "agy"]);
    env.path = Some(path_with(&["git", "sh"], &[bin]));
    let log = env.root.path().join("fake.log");
    env.vars = vec![
        ("E2E_GA_TOKEN".to_string(), SECRET.to_string()),
        (
            "FAKE_AGENT_LOG".to_string(),
            log.to_string_lossy().to_string(),
        ),
    ];
    env.forbidden = vec![SECRET.to_string()];
    std::fs::create_dir_all(env.home.join(".gemini")).unwrap();
    std::fs::write(gemini_settings(&env), GEMINI_FIXTURE).unwrap();

    let market = env.root.path().join("market");
    std::fs::create_dir_all(&market).unwrap();
    std::fs::write(
        market.join("gemini-extension.json"),
        r#"{"name":"demo","version":"1.0.0"}"#,
    )
    .unwrap();
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
fn gemini_and_antigravity_get_mcps_skills_and_plugins() {
    let (env, market) = setup();
    let status = env.ok(&["status"]);
    assert!(status.contains("gemini:\n"), "{status}");
    assert!(status.contains("antigravity:\n"), "{status}");
    assert!(status.contains("plugin demo: adicionar"), "{status}");

    env.ok(&["apply", "--yes"]);

    let text = std::fs::read_to_string(gemini_settings(&env)).unwrap();
    assert!(text.contains("// user comment that must survive"), "{text}");
    let g = servers(&gemini_settings(&env));
    assert_eq!(g["untouched"]["trust"], json!(true));
    assert_eq!(
        g["tool"],
        json!({"command": "node", "args": ["server.js"], "env": {"TOKEN": REF}})
    );
    assert_eq!(g["remote"]["httpUrl"], "https://example.com/mcp");
    assert_eq!(
        g["remote"]["headers"]["Authorization"],
        format!("Bearer {REF}")
    );

    let a = servers(&agy_config(&env));
    assert_eq!(a["tool"]["env"]["TOKEN"], SECRET);
    assert_eq!(a["remote"]["serverUrl"], "https://example.com/mcp");
    assert_eq!(
        a["remote"]["headers"]["Authorization"],
        format!("Bearer {SECRET}")
    );

    let gemini_skill = env.home.join(".gemini").join("skills").join("hello");
    let agy_skill = env
        .home
        .join(".gemini")
        .join("config")
        .join("skills")
        .join("hello");
    assert!(gemini_skill.join("SKILL.md").is_file());
    assert!(agy_skill.join("SKILL.md").is_file());

    let install = vec!["extensions", "install", market.as_str(), "--consent"];
    assert_eq!(calls(&env, "gemini").last().unwrap(), &install);
    assert_eq!(
        calls(&env, "agy").last().unwrap(),
        &["plugin", "install", market.as_str()]
    );
    let ext = env.home.join(".gemini").join("extensions").join("demo");
    assert!(ext.join("gemini-extension.json").is_file());
    let plugin = env
        .home
        .join(".gemini")
        .join("config")
        .join("plugins")
        .join("demo");
    assert!(plugin.join(".agsync-source.json").is_file());

    let status = env.ok(&["status"]);
    assert!(status.contains("gemini:\n  nada a mudar"), "{status}");
    assert!(status.contains("antigravity:\n  nada a mudar"), "{status}");

    env.push(&yaml(&market, false));
    env.ok(&["apply", "--yes"]);
    assert_eq!(
        calls(&env, "gemini").last().unwrap(),
        &["extensions", "uninstall", "demo"]
    );
    assert_eq!(
        calls(&env, "agy").last().unwrap(),
        &["plugin", "uninstall", "demo"]
    );
    assert!(!ext.exists() && !plugin.exists());
    let g = servers(&gemini_settings(&env));
    assert!(!g.contains_key("remote") && g.contains_key("untouched"));
    assert!(!servers(&agy_config(&env)).contains_key("remote"));
    let text = std::fs::read_to_string(gemini_settings(&env)).unwrap();
    assert!(text.contains("// user comment that must survive"), "{text}");

    let status = env.ok(&["status"]);
    assert!(status.contains("gemini:\n  nada a mudar"), "{status}");
    assert!(status.contains("antigravity:\n  nada a mudar"), "{status}");
}
