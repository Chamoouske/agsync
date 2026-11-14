//! End to end for `plugins:` with the real `claude` and `opencode` CLIs and a local git
//! marketplace, every dir temporary.
//!
//! Skipped (with a message) when the CLI is not in PATH.

mod common;

use std::path::{Path, PathBuf};

use common::{Env, claude_missing, fake_bin, git, path_with};
use serde_json::Value;

fn write_plugin(root: &Path, name: &str, version: &str) {
    let dir = root.join("plugins").join(name).join(".claude-plugin");
    std::fs::create_dir_all(&dir).unwrap();
    let json = format!(r#"{{"name":"{name}","version":"{version}","description":"test"}}"#);
    std::fs::write(dir.join("plugin.json"), json).unwrap();
}

fn write_marketplace(root: &Path, plugins: &[(&str, &str)]) {
    let dir = root.join(".claude-plugin");
    std::fs::create_dir_all(&dir).unwrap();
    let list: Vec<String> = plugins
        .iter()
        .map(|(name, version)| {
            write_plugin(root, name, version);
            format!(
                r#"{{"name":"{name}","source":"./plugins/{name}","description":"test","version":"{version}"}}"#
            )
        })
        .collect();
    let json = format!(
        r#"{{"name":"test-market","owner":{{"name":"test"}},"plugins":[{}]}}"#,
        list.join(",")
    );
    std::fs::write(dir.join("marketplace.json"), json).unwrap();
}

fn commit(dir: &Path, message: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
}

fn market_yaml(market: &Path, ref_: &str) -> String {
    let git = market.to_string_lossy().replace('\\', "/");
    format!(
        "version: 1\nprofiles: [work, personal]\nmcps: {{}}\nplugins:\n  - name: p1\n    marketplace: {{git: '{git}', ref: {ref_}}}\n"
    )
}

fn with_mcp(yaml: &str, name: &str) -> String {
    yaml.replace(
        "mcps: {}\n",
        &format!("mcps:\n  {name}:\n    command: node\n    args: [\"x.js\"]\n"),
    )
}

fn without_plugins(yaml: &str) -> String {
    let start = yaml.find("plugins:").unwrap();
    format!("{}plugins: []\n", &yaml[..start])
}

fn claude_plugins(env: &Env) -> Vec<serde_json::Value> {
    let text = env.claude(&["plugin", "list", "--json"]);
    let all: Vec<Value> = serde_json::from_str(&text).unwrap();
    all.into_iter()
        .filter(|p| p["scope"] != "session")
        .collect()
}

fn has_plugin(env: &Env, id: &str) -> bool {
    claude_plugins(env).iter().any(|p| p["id"] == id)
}

fn plugin_version(env: &Env, id: &str) -> String {
    claude_plugins(env)
        .into_iter()
        .find(|p| p["id"] == id)
        .map(|p| p["version"].as_str().unwrap_or_default().to_string())
        .unwrap_or_default()
}

#[test]
fn plugins_end_to_end_with_real_claude() {
    if claude_missing("plugins_end_to_end_with_real_claude") {
        return;
    }
    let mut env = Env::new();
    env.path = Some(path_with(&["git", "claude", "sh"], &[]));
    let market = env.root.path().join("market");
    std::fs::create_dir_all(&market).unwrap();
    write_marketplace(
        &market,
        &[("p1", "1.0.0"), ("p2", "1.0.0"), ("inactive", "1.0.0")],
    );
    let local = market.to_string_lossy().to_string();
    git(&market, &["init", "-q", "-b", "main"]);
    commit(&market, "init");

    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push(&market_yaml(&market, "v1"));
    let url = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &url, "--profile", "work"]);

    // A plugin installed by hand in the same marketplace, outside agsync.
    env.claude(&[
        "plugin",
        "marketplace",
        "add",
        &local,
        "--scope",
        "user",
        "--json",
    ]);
    env.claude(&[
        "plugin",
        "install",
        "p2@test-market",
        "--scope",
        "user",
        "--json",
        "-y",
    ]);

    let shown = market.to_string_lossy().replace('\\', "/");
    let status = env.ok(&["status"]);
    assert!(
        status.contains(&format!("plugin p1: adicionar [{shown}]")),
        "{status}"
    );
    assert!(!status.contains("inactive"), "{status}");

    let diff = env.ok(&["diff"]);
    assert!(diff.contains("plugin p1: adicionar"), "{diff}");
    env.ok(&["check"]);
    let pending = env.pending().expect("check sees the new plugin");
    assert_eq!(pending["agents"]["claude"]["added"][0], "plugin p1");
    assert!(
        env.notifications()
            .last()
            .unwrap()
            .contains("1 mudanca pendente"),
        "{:?}",
        env.notifications()
    );

    let applied = env.ok(&["apply", "--yes"]);
    assert!(
        applied.contains("claude: plugin p1: ok (adicionar)"),
        "{applied}"
    );
    assert!(has_plugin(&env, "p1@test-market"));
    assert!(has_plugin(&env, "p2@test-market"));
    assert!(!has_plugin(&env, "inactive@test-market"));
    let state = env.state();
    assert!(state.contains("\"plugin\": \"p1@test-market\""), "{state}");
    assert!(!state.contains("p2@test-market"), "{state}");
    assert!(env.ok(&["status"]).contains("claude:\n  nada a mudar"));

    // Ref change: agsync reinstalls the plugin, and the manual one stays.
    write_marketplace(
        &market,
        &[("p1", "1.1.0"), ("p2", "1.0.0"), ("inactive", "1.0.0")],
    );
    commit(&market, "p1 1.1.0");
    env.push(&market_yaml(&market, "v2"));
    env.ok(&["check"]);
    let pending = env.pending().expect("check sees the new ref");
    assert_eq!(pending["agents"]["claude"]["changed"][0], "plugin p1");
    assert!(env.ok(&["status"]).contains("plugin p1: atualizar"));
    env.ok(&["apply", "--yes"]);
    assert_eq!(plugin_version(&env, "p1@test-market"), "1.1.0");
    assert!(has_plugin(&env, "p2@test-market"));
    assert!(env.ok(&["status"]).contains("claude:\n  nada a mudar"));

    // Declared both as plugin and as MCP: the diff warns, the MCP is still written.
    let clash = with_mcp(&market_yaml(&market, "v2"), "p1");
    env.push(&clash);
    let diff = env.ok(&["diff"]);
    assert!(diff.contains("mcp p1: adicionar"), "{diff}");
    assert!(
        diff.contains("aviso: plugin p1 (")
            && diff.contains(") tambem esta em mcps para claude; pode duplicar no agente"),
        "{diff}"
    );
    env.ok(&["apply", "--yes"]);

    // Removed from the YAML: uninstalled, manual plugin untouched.
    env.push(&without_plugins(&market_yaml(&market, "v2")));
    env.ok(&["check"]);
    let pending = env.pending().expect("check sees the removed plugin");
    let removed = pending["agents"]["claude"]["removed"].as_array().unwrap();
    assert!(removed.iter().any(|item| item == "plugin p1"), "{pending}");
    let diff = env.ok(&["diff"]);
    assert!(diff.contains("plugin p1: remover"), "{diff}");
    env.ok(&["apply", "--yes"]);
    assert!(!has_plugin(&env, "p1@test-market"));
    assert!(has_plugin(&env, "p2@test-market"));
    assert!(!env.state().contains("p1@test-market"));

    // `agsync uninstall` removes the plugins it tracks and leaves the manual one.
    env.push(&market_yaml(&market, "v2"));
    env.ok(&["apply", "--yes"]);
    assert!(has_plugin(&env, "p1@test-market"));
    env.ok(&["uninstall", "--yes"]);
    assert!(!has_plugin(&env, "p1@test-market"));
    assert!(has_plugin(&env, "p2@test-market"));
    assert!(!env.data.join("state.json").exists());
}

fn opencode_plugin_repo(root: &Path) -> PathBuf {
    std::fs::create_dir_all(root).unwrap();
    let package = r#"{"name":"oc-plugin-p1","version":"1.0.0","type":"module"}"#;
    std::fs::write(root.join("package.json"), package).unwrap();
    std::fs::write(
        root.join("index.js"),
        "export const P1 = async () => ({})\n",
    )
    .unwrap();
    git(root, &["init", "-q", "-b", "main"]);
    commit(root, "init");
    root.to_path_buf()
}

fn opencode_yaml(git_url: &str, ref_: &str) -> String {
    format!(
        "version: 1\nprofiles: [work]\nmcps: {{}}\nplugins:\n  - name: p1\n    agents: [opencode]\n    marketplace: {{git: '{git_url}', ref: {ref_}}}\n"
    )
}

#[test]
fn plugins_end_to_end_with_real_opencode() {
    if agsync::which::find("opencode").is_none() {
        eprintln!("SKIP plugins_end_to_end_with_real_opencode: `opencode` nao esta no PATH");
        return;
    }
    let mut env = Env::new();
    env.path = Some(path_with(&["git", "opencode", "sh"], &[]));
    let repo = opencode_plugin_repo(&env.root.path().join("oc-market"));
    let url = format!("file://{}", repo.to_string_lossy().replace('\\', "/"));

    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push(&opencode_yaml(&url, "main"));
    let remote = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &remote, "--profile", "work"]);

    let package = format!("git+{url}#main");
    let status = env.ok(&["status"]);
    assert!(
        status.contains("opencode:\n  plugin p1: adicionar"),
        "{status}"
    );
    env.ok(&["apply", "--yes"]);
    let config = std::fs::read_to_string(env.opencode_dir.join("opencode.json")).unwrap();
    assert!(config.contains(&package), "{config}");
    assert!(
        env.state().contains(&package.replace('\\', "\\\\")),
        "{:?}",
        env.state()
    );

    git(&repo, &["tag", "v2"]);
    env.push(&opencode_yaml(&url, "v2"));
    assert!(env.ok(&["status"]).contains("plugin p1: atualizar"));
    env.ok(&["apply", "--yes"]);
    let config = std::fs::read_to_string(env.opencode_dir.join("opencode.json")).unwrap();
    assert!(config.contains(&format!("git+{url}#v2")), "{config}");
    assert!(!config.contains("#main"), "{config}");

    env.push("version: 1\nprofiles: [work]\nmcps: {}\nplugins: []\n");
    assert!(env.ok(&["status"]).contains("plugin p1: remover"));
    env.ok(&["apply", "--yes"]);
    let config = std::fs::read_to_string(env.opencode_dir.join("opencode.json")).unwrap();
    assert!(!config.contains("oc-market"), "{config}");
}

#[test]
fn plugin_for_agent_without_support_is_skipped() {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex"]);
    env.path = Some(path_with(&["git", "sh"], &[bin]));
    let log = env.root.path().join("fake.log");
    env.vars = vec![(
        "FAKE_AGENT_LOG".to_string(),
        log.to_string_lossy().to_string(),
    )];
    git(&env.remote, &["init", "-q", "-b", "main"]);
    let yaml = "version: 1\nprofiles: [work]\nmcps: {}\nplugins:\n  - name: p1\n    agents: [codex]\n    marketplace: {git: 'https://x/market.git', ref: v1}\n";
    env.push(yaml);
    let url = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &url, "--profile", "work"]);

    let status = env.ok(&["status"]);
    assert!(status.contains("codex:\n  nada a mudar"), "{status}");
    assert!(
        status.contains("pulados:")
            && status.contains("codex: plugins: CLI sem comandos nao interativos de plugins"),
        "{status}"
    );
    env.ok(&["apply", "--yes"]);
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(!calls.contains("plugin"), "{calls}");
    assert!(env.ok(&["status"]).contains("codex: plugins: CLI sem"));
}
