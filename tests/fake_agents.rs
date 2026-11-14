//! End to end for the Codex and Hermes adapters against fake CLIs.
//!
//! Neither agent is installed on the dev machine, so `examples/fake_agent.rs` stands in
//! for `codex` and `hermes`: it logs every call and applies the expected effect to the
//! temp config file. `PATH` only holds the fakes, git and sh, so no real agent runs.

mod common;

use std::path::PathBuf;
use std::process::Command;

use common::{Env, count, git, path_with};
use serde_json::{Value, json};

const ENV_SECRET: &str = "fake-env-secret-333";
const FILE_SECRET: &str = "fake-file-secret-444";
const ENV_REF: &str = "${E2E_FAKE_ENV}";

const CODEX_FIXTURE: &str = r#"# user comment that must survive
model = "o3"

[profiles.fast]
model = "o4-mini" # trailing comment

[mcp_servers.untouched]
command = "manual-tool"
args = ["keep"]

[mcp_servers.manual]
command = "manual-tool"
args = ["mine"]
"#;

const HERMES_FIXTURE: &str = "# user comment
model: hermes-model
toolsets: [web, terminal]
mcp_servers:
  untouched:
    command: manual-tool
    args: [keep]
  manual:
    command: manual-tool
    args: [mine]
";

/// Builds the fake agent example and copies it as each name into a fresh bin dir.
fn fake_bin(env: &Env, names: &[&str]) -> PathBuf {
    let built = Command::new(env!("CARGO"))
        .args(["build", "--quiet", "--example", "fake_agent"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .unwrap();
    assert!(built.success(), "cargo build --example fake_agent falhou");
    let deps = std::env::current_exe().unwrap();
    let exe = deps
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("examples")
        .join(format!("fake_agent{}", std::env::consts::EXE_SUFFIX));
    let bin = env.root.path().join("fakebin");
    std::fs::create_dir_all(&bin).unwrap();
    for name in names {
        let dest = bin.join(format!("{name}{}", std::env::consts::EXE_SUFFIX));
        std::fs::copy(&exe, dest).unwrap();
    }
    bin
}

/// Calls the fake `agent` received, oldest first.
fn calls(env: &Env, agent: &str) -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(env.root.path().join("fake.log")).unwrap_or_default();
    text.lines()
        .map(|line| serde_json::from_str::<Vec<String>>(line).unwrap())
        .filter(|call| call[0] == agent)
        .map(|call| call[1..].to_vec())
        .collect()
}

fn last_calls(env: &Env, agent: &str, n: usize) -> Vec<Vec<String>> {
    let all = calls(env, agent);
    all[all.len().saturating_sub(n)..].to_vec()
}

fn strs(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| w.to_string()).collect()
}

fn codex_text(env: &Env) -> String {
    std::fs::read_to_string(env.codex_dir.join("config.toml")).unwrap()
}

fn codex_servers(env: &Env) -> toml::Table {
    let root: toml::Table = toml::from_str(&codex_text(env)).unwrap();
    root["mcp_servers"].as_table().unwrap().clone()
}

fn hermes_root(env: &Env) -> Value {
    let text = std::fs::read_to_string(env.hermes_dir.join("config.yaml")).unwrap();
    serde_saphyr::from_str(&text).unwrap()
}

fn hermes_servers(env: &Env) -> serde_json::Map<String, Value> {
    hermes_root(env)["mcp_servers"].as_object().unwrap().clone()
}

fn yaml(plain_arg: Option<&str>, extra: &str) -> String {
    let plain = match plain_arg {
        Some(arg) => {
            format!("  plain:\n    command: node\n    args: [\"{arg}\"]\n    env: {{MODE: fast}}\n")
        }
        None => String::new(),
    };
    format!(
        r#"version: 1
profiles: [work]
mcps:
{plain}  envref:
    command: node
    env: {{E2E_FAKE_ENV: "{ENV_REF}"}}
  renamed:
    command: node
    env: {{TOKEN: "{ENV_REF}"}}
  fileval:
    command: node
    env: {{TOKEN: "${{E2E_FAKE_FILE}}"}}
  remote:
    url: https://example.com/mcp
    headers: {{Authorization: "Bearer {ENV_REF}"}}
  remotefile:
    url: https://example.com/mcp2
    headers: {{X-Key: "${{E2E_FAKE_FILE}}", X-Env: "{ENV_REF}", X-Team: t1}}
{extra}skills:
  - path: skills/hello
"#
    )
}

fn setup() -> Env {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex", "hermes"]);
    env.path = Some(path_with(&["git", "sh"], &[bin]));
    let log = env.root.path().join("fake.log");
    env.vars = vec![
        ("E2E_FAKE_ENV".to_string(), ENV_SECRET.to_string()),
        (
            "FAKE_AGENT_LOG".to_string(),
            log.to_string_lossy().to_string(),
        ),
    ];
    env.forbidden = vec![ENV_SECRET.to_string(), FILE_SECRET.to_string()];
    std::fs::write(env.codex_dir.join("config.toml"), CODEX_FIXTURE).unwrap();
    std::fs::write(env.hermes_dir.join("config.yaml"), HERMES_FIXTURE).unwrap();
    git(&env.remote, &["init", "-q", "-b", "main"]);
    let skill = env.remote.join("skills").join("hello");
    std::fs::create_dir_all(&skill).unwrap();
    std::fs::write(
        skill.join("SKILL.md"),
        "---\nname: hello\ndescription: t\n---\nhi\n",
    )
    .unwrap();
    env.push(&yaml(Some("v1.js"), ""));
    let url = env.remote.to_string_lossy().to_string();
    env.ok(&["init", &url, "--profile", "work"]);
    let (ok, out) = env.agsync_with_input(
        &["secret", "set", "E2E_FAKE_FILE"],
        Some(&format!("{FILE_SECRET}\n")),
    );
    assert!(ok, "{out}");
    env
}

fn codex_add_plain(arg: &str) -> Vec<String> {
    strs(&[
        "mcp",
        "add",
        "plain",
        "--env",
        "MODE=fast",
        "--",
        "node",
        arg,
    ])
}

fn hermes_set_plain(arg: &str) -> Vec<String> {
    let entry = json!({"command": "node", "args": [arg], "env": {"MODE": "fast"}});
    strs(&["config", "set", "mcp_servers.plain", &entry.to_string()])
}

fn assert_first_apply(env: &Env) {
    assert!(calls(env, "codex").contains(&codex_add_plain("v1.js")));
    assert!(!calls(env, "codex").iter().flatten().any(|w| w == "envref"));
    let s = codex_servers(env);
    let names = toml::Value::Array(vec!["E2E_FAKE_ENV".into()]);
    assert_eq!(s["envref"]["env_vars"], names);
    assert!(s["envref"].get("env").is_none());
    assert_eq!(s["renamed"]["env"]["TOKEN"].as_str(), Some(ENV_SECRET));
    assert_eq!(s["fileval"]["env"]["TOKEN"].as_str(), Some(FILE_SECRET));
    assert_eq!(
        s["untouched"]["args"],
        toml::Value::Array(vec!["keep".into()])
    );
    let add_remote = strs(&[
        "mcp",
        "add",
        "remote",
        "--url",
        "https://example.com/mcp",
        "--bearer-token-env-var",
        "E2E_FAKE_ENV",
    ]);
    assert!(calls(env, "codex").contains(&add_remote));
    assert!(
        !calls(env, "codex")
            .iter()
            .flatten()
            .any(|w| w == "remotefile")
    );
    assert_eq!(
        s["remote"]["bearer_token_env_var"].as_str(),
        Some("E2E_FAKE_ENV")
    );
    let remotefile = &s["remotefile"];
    assert_eq!(remotefile["url"].as_str(), Some("https://example.com/mcp2"));
    assert_eq!(
        remotefile["http_headers"]["X-Key"].as_str(),
        Some(FILE_SECRET)
    );
    assert_eq!(remotefile["http_headers"]["X-Team"].as_str(), Some("t1"));
    let from_env = remotefile["env_http_headers"]["X-Env"].as_str();
    assert_eq!(from_env, Some("E2E_FAKE_ENV"));
    let text = codex_text(env);
    assert!(
        text.starts_with("# user comment that must survive"),
        "{text}"
    );
    assert!(
        text.contains("model = \"o4-mini\" # trailing comment"),
        "{text}"
    );

    assert!(calls(env, "hermes").contains(&hermes_set_plain("v1.js")));
    let h = hermes_servers(env);
    assert_eq!(h["envref"]["env"]["E2E_FAKE_ENV"], ENV_REF);
    assert_eq!(h["renamed"]["env"]["TOKEN"], ENV_REF);
    assert_eq!(h["fileval"]["env"]["TOKEN"], FILE_SECRET);
    assert_eq!(h["untouched"]["args"], json!(["keep"]));
    let bearer = format!("Bearer {ENV_REF}");
    assert_eq!(h["remote"]["url"], "https://example.com/mcp");
    assert_eq!(h["remote"]["headers"]["Authorization"], bearer);
    assert_eq!(h["remotefile"]["headers"]["X-Key"], FILE_SECRET);
    assert_eq!(h["remotefile"]["headers"]["X-Env"], ENV_REF);
    assert_eq!(h["remotefile"]["headers"]["X-Team"], "t1");
    let root = hermes_root(env);
    assert_eq!(root["model"], "hermes-model");
    assert_eq!(root["toolsets"], json!(["web", "terminal"]));

    let agents_skill = env.home.join(".agents").join("skills").join("hello");
    assert!(agents_skill.join("SKILL.md").is_file());
    let hermes_skill = env.hermes_dir.join("skills").join("hello");
    assert!(hermes_skill.join("SKILL.md").is_file());
    let state = env.state();
    assert!(
        !state.contains(ENV_SECRET) && !state.contains(FILE_SECRET),
        "{state}"
    );
}

#[test]
fn codex_and_hermes_against_fake_clis() {
    let env = setup();
    let status = env.ok(&["status"]);
    assert!(status.contains("codex:\n"), "{status}");
    assert!(status.contains("hermes:\n"), "{status}");
    assert!(status.contains("mcp plain: adicionar"), "{status}");
    assert!(
        status.contains("claude: claude nao encontrado no PATH"),
        "{status}"
    );

    env.ok(&["apply", "--yes"]);
    assert_first_apply(&env);
    let status = env.ok(&["status"]);
    assert!(status.contains("codex:\n  nada a mudar"), "{status}");
    assert!(status.contains("hermes:\n  nada a mudar"), "{status}");

    env.push(&yaml(Some("v2.js"), ""));
    assert!(env.ok(&["status"]).contains("mcp plain: atualizar"));
    env.ok(&["apply", "--yes"]);
    let codex_update = vec![strs(&["mcp", "remove", "plain"]), codex_add_plain("v2.js")];
    assert_eq!(last_calls(&env, "codex", 2), codex_update);
    let hermes_update = vec![
        strs(&["config", "unset", "mcp_servers.plain"]),
        hermes_set_plain("v2.js"),
    ];
    assert_eq!(last_calls(&env, "hermes", 2), hermes_update);

    let hacked = codex_text(&env).replace("[\"E2E_FAKE_ENV\"]", "[\"HACK\"]");
    std::fs::write(env.codex_dir.join("config.toml"), hacked).unwrap();
    let mut root = hermes_root(&env);
    root["mcp_servers"]["envref"]["command"] = json!("hacked");
    std::fs::write(env.hermes_dir.join("config.yaml"), root.to_string()).unwrap();
    let status = env.ok(&["status"]);
    assert_eq!(
        status.matches("mcp envref: desvio manual").count(),
        2,
        "{status}"
    );
    let backups = count(&env.data.join("backups"));
    env.ok(&["apply", "--yes"]);
    let env_vars = &codex_servers(&env)["envref"]["env_vars"];
    assert_eq!(env_vars, &toml::Value::Array(vec!["E2E_FAKE_ENV".into()]));
    assert_eq!(hermes_servers(&env)["envref"]["command"], "node");
    assert!(count(&env.data.join("backups")) >= backups + 2);

    let manual = "  manual:\n    command: node\n    args: [\"repo.js\"]\n";
    env.push(&yaml(Some("v2.js"), manual));
    let status = env.ok(&["status"]);
    assert_eq!(
        status.matches("mcp manual: conflito").count(),
        2,
        "{status}"
    );
    env.ok(&["apply", "--yes"]);
    let mine = toml::Value::Array(vec!["mine".into()]);
    assert_eq!(codex_servers(&env)["manual"]["args"], mine);
    assert_eq!(hermes_servers(&env)["manual"]["args"], json!(["mine"]));
    let adopted = env.ok(&["apply", "--yes", "--adopt"]);
    assert_eq!(adopted.matches("ok (assumir").count(), 2, "{adopted}");
    let repo = toml::Value::Array(vec!["repo.js".into()]);
    assert_eq!(codex_servers(&env)["manual"]["args"], repo);
    assert_eq!(hermes_servers(&env)["manual"]["args"], json!(["repo.js"]));

    env.push(&yaml(None, manual));
    assert!(env.ok(&["status"]).contains("mcp plain: remover"));
    env.ok(&["apply", "--yes"]);
    let codex_remove = vec![strs(&["mcp", "remove", "plain"])];
    assert_eq!(last_calls(&env, "codex", 1), codex_remove);
    let hermes_remove = vec![strs(&["config", "unset", "mcp_servers.plain"])];
    assert_eq!(last_calls(&env, "hermes", 1), hermes_remove);
    let s = codex_servers(&env);
    assert!(!s.contains_key("plain"));
    assert!(s.contains_key("untouched") && s.contains_key("envref"));
    assert!(codex_text(&env).starts_with("# user comment that must survive"));
    let h = hermes_servers(&env);
    assert!(!h.contains_key("plain"));
    assert!(h.contains_key("untouched") && h.contains_key("envref"));
    assert_eq!(hermes_root(&env)["model"], "hermes-model");
    let status = env.ok(&["status"]);
    assert!(status.contains("codex:\n  nada a mudar"), "{status}");
    assert!(status.contains("hermes:\n  nada a mudar"), "{status}");
}
