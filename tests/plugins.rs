//! End to end for `plugins:` with the real `claude` and `opencode` CLIs, the fake
//! `codex`, and a local git marketplace, every dir temporary.
//!
//! Tests of a real CLI are skipped (with a message) when it is not in PATH.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

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

/// HTTPS address the tests give the marketplace; git maps it to a local repo through
/// `url.<base>.insteadOf`, because `claude plugin marketplace add` rejects `file://`.
const REMOTE_MARKET: &str = "https://agsync.test/market.git";

fn remote_market_yaml(ref_: &str) -> String {
    format!(
        "version: 1\nprofiles: [work]\nmcps: {{}}\nplugins:\n  - name: p1\n    marketplace: {{git: '{REMOTE_MARKET}', ref: {ref_}}}\n  - name: p2\n    marketplace: {{git: '{REMOTE_MARKET}', ref: {ref_}}}\n"
    )
}

fn release(market: &Path, version: &str, tag: &str) {
    write_marketplace(market, &[("p1", version), ("p2", version), ("p3", "1.0.0")]);
    commit(market, version);
    git(market, &["tag", tag]);
}

fn marketplace_ref(env: &Env) -> String {
    let text = env.claude(&["plugin", "marketplace", "list", "--json"]);
    let all: Vec<Value> = serde_json::from_str(&text).unwrap();
    all.iter()
        .find(|m| m["name"] == "test-market")
        .map(|m| m["ref"].as_str().unwrap_or_default().to_string())
        .unwrap_or_default()
}

#[test]
fn plugins_switch_ref_of_remote_marketplace_with_real_claude() {
    if claude_missing("plugins_switch_ref_of_remote_marketplace_with_real_claude") {
        return;
    }
    let mut env = Env::new();
    env.path = Some(path_with(&["git", "claude", "sh"], &[]));
    let market = env.root.path().join("market");
    std::fs::create_dir_all(&market).unwrap();
    git(&market, &["init", "-q", "-b", "main"]);
    release(&market, "1.0.0", "v1");
    let base = format!("file:///{}", market.to_string_lossy().replace('\\', "/"));
    env.vars = vec![
        ("GIT_CONFIG_COUNT".to_string(), "1".to_string()),
        (
            "GIT_CONFIG_KEY_0".to_string(),
            format!("url.{base}.insteadOf"),
        ),
        ("GIT_CONFIG_VALUE_0".to_string(), REMOTE_MARKET.to_string()),
    ];
    assert!(!agsync::plugin::is_local_marketplace(REMOTE_MARKET));

    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push(&remote_market_yaml("v1"));
    let url = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &url, "--profile", "work"]);
    env.ok(&["apply", "--yes"]);
    assert_eq!(marketplace_ref(&env), "v1");
    assert_eq!(plugin_version(&env, "p1@test-market"), "1.0.0");
    assert_eq!(plugin_version(&env, "p2@test-market"), "1.0.0");

    release(&market, "1.1.0", "v2");
    env.push(&remote_market_yaml("v2"));
    assert!(env.ok(&["status"]).contains("plugin p1: atualizar"));
    env.ok(&["apply", "--yes"]);
    assert_eq!(marketplace_ref(&env), "v2");
    assert_eq!(plugin_version(&env, "p1@test-market"), "1.1.0");
    assert_eq!(plugin_version(&env, "p2@test-market"), "1.1.0");
    assert!(env.ok(&["status"]).contains("claude:\n  nada a mudar"));

    env.claude(&[
        "plugin",
        "install",
        "p3@test-market",
        "--scope",
        "user",
        "--json",
        "-y",
    ]);
    release(&market, "1.2.0", "v3");
    env.push(&remote_market_yaml("v3"));
    let (ok, out) = env.agsync(&["apply", "--yes"]);
    assert!(!ok, "{out}");
    assert!(out.contains("troque a ref a mao"), "{out}");
    assert!(out.contains("p3@test-market"), "{out}");
    assert_eq!(marketplace_ref(&env), "v2");
    assert_eq!(plugin_version(&env, "p1@test-market"), "1.1.0");
    assert!(has_plugin(&env, "p3@test-market"));
}

fn codex(env: &Env, bin: &Path, args: &[&str]) -> String {
    let exe = bin.join(format!("codex{}", std::env::consts::EXE_SUFFIX));
    let mut cmd = Command::new(exe);
    cmd.args(args);
    env.isolate(&mut cmd);
    let out = cmd.output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    assert!(out.status.success(), "codex {args:?}: {text}");
    text
}

fn codex_version(env: &Env, bin: &Path, id: &str) -> String {
    let list: Value =
        serde_json::from_str(&codex(env, bin, &["plugin", "list", "--json"])).unwrap();
    list["installed"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["pluginId"] == id)
        .map(|p| p["version"].as_str().unwrap_or_default().to_string())
        .unwrap_or_default()
}

fn codex_marketplace_ref(env: &Env) -> String {
    let text = std::fs::read_to_string(env.codex_dir.join("config.toml")).unwrap();
    let config: toml::Table = toml::from_str(&text).unwrap();
    config["marketplaces"]["test-market"]["ref"]
        .as_str()
        .unwrap_or_default()
        .to_string()
}

#[test]
fn plugins_end_to_end_with_fake_codex() {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex"]);
    env.path = Some(path_with(&["git", "sh"], std::slice::from_ref(&bin)));
    let market = env.root.path().join("market");
    std::fs::create_dir_all(&market).unwrap();
    git(&market, &["init", "-q", "-b", "main"]);
    release(&market, "1.0.0", "v1");
    let base = format!("file:///{}", market.to_string_lossy().replace('\\', "/"));
    env.vars = vec![
        ("GIT_CONFIG_COUNT".to_string(), "1".to_string()),
        (
            "GIT_CONFIG_KEY_0".to_string(),
            format!("url.{base}.insteadOf"),
        ),
        ("GIT_CONFIG_VALUE_0".to_string(), REMOTE_MARKET.to_string()),
    ];

    git(&env.remote, &["init", "-q", "-b", "main"]);
    env.push(&remote_market_yaml("v1"));
    let url = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &url, "--profile", "work"]);

    let status = env.ok(&["status"]);
    assert!(
        status.contains("codex:\n  plugin p1: adicionar"),
        "{status}"
    );
    let applied = env.ok(&["apply", "--yes"]);
    assert!(
        applied.contains("codex: plugin p1: ok (adicionar)"),
        "{applied}"
    );
    assert_eq!(codex_marketplace_ref(&env), "v1");
    assert_eq!(codex_version(&env, &bin, "p1@test-market"), "1.0.0");
    assert_eq!(codex_version(&env, &bin, "p2@test-market"), "1.0.0");
    assert!(env.state().contains("\"plugin\": \"p1@test-market\""));
    assert!(env.ok(&["status"]).contains("codex:\n  nada a mudar"));

    release(&market, "1.1.0", "v2");
    env.push(&remote_market_yaml("v2"));
    assert!(env.ok(&["status"]).contains("plugin p1: atualizar"));
    env.ok(&["apply", "--yes"]);
    assert_eq!(codex_marketplace_ref(&env), "v2");
    assert_eq!(codex_version(&env, &bin, "p1@test-market"), "1.1.0");
    assert_eq!(codex_version(&env, &bin, "p2@test-market"), "1.1.0");
    assert!(env.ok(&["status"]).contains("codex:\n  nada a mudar"));

    env.push(&remote_market_yaml("missing"));
    let (ok, out) = env.agsync(&["apply", "--yes"]);
    assert!(!ok, "{out}");
    assert!(out.contains("voltou para a ref antiga"), "{out}");
    assert_eq!(codex_marketplace_ref(&env), "v2");
    assert_eq!(codex_version(&env, &bin, "p1@test-market"), "1.1.0");
    assert!(env.ok(&["status"]).contains("plugin p1: atualizar"));

    codex(&env, &bin, &["plugin", "add", "p3@test-market", "--json"]);
    release(&market, "1.2.0", "v3");
    env.push(&remote_market_yaml("v3"));
    let (ok, out) = env.agsync(&["apply", "--yes"]);
    assert!(!ok, "{out}");
    assert!(out.contains("troque a ref a mao"), "{out}");
    assert!(out.contains("p3@test-market"), "{out}");
    assert_eq!(codex_marketplace_ref(&env), "v2");
    codex(
        &env,
        &bin,
        &["plugin", "remove", "p3@test-market", "--json"],
    );

    env.push(&without_plugins(&remote_market_yaml("v3")));
    assert!(env.ok(&["status"]).contains("plugin p1: remover"));
    env.ok(&["apply", "--yes"]);
    assert_eq!(codex_version(&env, &bin, "p1@test-market"), "");
    assert_eq!(codex_version(&env, &bin, "p2@test-market"), "");
    assert!(!env.state().contains("p1@test-market"));

    env.push(&remote_market_yaml("v3"));
    env.ok(&["apply", "--yes"]);
    assert_eq!(codex_version(&env, &bin, "p1@test-market"), "1.2.0");
    env.ok(&["uninstall", "--yes"]);
    assert_eq!(codex_version(&env, &bin, "p1@test-market"), "");
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
    let bin = fake_bin(&env, &["hermes"]);
    env.path = Some(path_with(&["git", "sh"], &[bin]));
    let log = env.root.path().join("fake.log");
    env.vars = vec![(
        "FAKE_AGENT_LOG".to_string(),
        log.to_string_lossy().to_string(),
    )];
    git(&env.remote, &["init", "-q", "-b", "main"]);
    let yaml = "version: 1\nprofiles: [work]\nmcps: {}\nplugins:\n  - name: p1\n    agents: [hermes]\n    marketplace: {git: 'https://x/market.git', ref: v1}\n";
    env.push(yaml);
    let url = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &url, "--profile", "work"]);

    let status = env.ok(&["status"]);
    assert!(status.contains("hermes:\n  nada a mudar"), "{status}");
    assert!(
        status.contains("pulados:")
            && status.contains("hermes: plugins: CLI sem comandos nao interativos de plugins"),
        "{status}"
    );
    env.ok(&["apply", "--yes"]);
    let calls = std::fs::read_to_string(&log).unwrap_or_default();
    assert!(!calls.contains("plugin"), "{calls}");
    assert!(env.ok(&["status"]).contains("hermes: plugins: CLI sem"));
}
