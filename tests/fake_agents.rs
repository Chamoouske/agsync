//! End to end for the Codex and Hermes adapters against fake CLIs.
//!
//! Neither agent is installed on the dev machine, so `examples/fake_agent.rs` stands in
//! for `codex` and `hermes`: it logs every call and applies the expected effect to the
//! temp config file. `PATH` only holds the fakes, git and sh, so no real agent runs.

mod common;

use std::path::PathBuf;
use std::process::Command;

use common::{Env, count, git, path_with};

const ENV_SECRET: &str = "fake-env-secret-333";
const FILE_SECRET: &str = "fake-file-secret-444";

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

fn calls(env: &Env) -> Vec<Vec<String>> {
    let text = std::fs::read_to_string(env.root.path().join("fake.log")).unwrap_or_default();
    text.lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn last_calls(env: &Env, n: usize) -> Vec<Vec<String>> {
    let all = calls(env);
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
    env: {{E2E_FAKE_ENV: "${{E2E_FAKE_ENV}}"}}
  renamed:
    command: node
    env: {{TOKEN: "${{E2E_FAKE_ENV}}"}}
  fileval:
    command: node
    env: {{TOKEN: "${{E2E_FAKE_FILE}}"}}
{extra}skills:
  - path: skills/hello
"#
    )
}

fn setup() -> Env {
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex"]);
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

#[test]
fn codex_and_hermes_against_fake_clis() {
    let env = setup();
    let status = env.ok(&["status"]);
    assert!(status.contains("codex:\n"), "{status}");
    assert!(status.contains("mcp plain: adicionar"), "{status}");
    assert!(
        status.contains("claude: claude nao encontrado no PATH"),
        "{status}"
    );

    env.ok(&["apply", "--yes"]);
    let add_plain = strs(&[
        "mcp",
        "add",
        "plain",
        "--env",
        "MODE=fast",
        "--",
        "node",
        "v1.js",
    ]);
    assert!(calls(&env).contains(&add_plain), "{:?}", calls(&env));
    assert!(!calls(&env).iter().flatten().any(|w| w == "envref"));
    let s = codex_servers(&env);
    assert_eq!(
        s["envref"]["env_vars"],
        toml::Value::Array(vec!["E2E_FAKE_ENV".into()])
    );
    assert!(s["envref"].get("env").is_none());
    assert_eq!(s["renamed"]["env"]["TOKEN"].as_str(), Some(ENV_SECRET));
    assert_eq!(s["fileval"]["env"]["TOKEN"].as_str(), Some(FILE_SECRET));
    let text = codex_text(&env);
    assert!(
        text.starts_with("# user comment that must survive"),
        "{text}"
    );
    assert!(
        text.contains("model = \"o4-mini\" # trailing comment"),
        "{text}"
    );
    assert_eq!(
        s["untouched"]["args"],
        toml::Value::Array(vec!["keep".into()])
    );
    assert!(
        env.home
            .join(".agents")
            .join("skills")
            .join("hello")
            .join("SKILL.md")
            .is_file()
    );
    let state = env.state();
    assert!(
        !state.contains(ENV_SECRET) && !state.contains(FILE_SECRET),
        "{state}"
    );
    assert!(env.ok(&["status"]).contains("codex:\n  nada a mudar"));

    env.push(&yaml(Some("v2.js"), ""));
    assert!(env.ok(&["status"]).contains("mcp plain: atualizar"));
    env.ok(&["apply", "--yes"]);
    let update = vec![
        strs(&["mcp", "remove", "plain"]),
        strs(&[
            "mcp",
            "add",
            "plain",
            "--env",
            "MODE=fast",
            "--",
            "node",
            "v2.js",
        ]),
    ];
    assert_eq!(last_calls(&env, 2), update);

    let hacked = codex_text(&env).replace("[\"E2E_FAKE_ENV\"]", "[\"HACK\"]");
    std::fs::write(env.codex_dir.join("config.toml"), hacked).unwrap();
    let status = env.ok(&["status"]);
    assert!(status.contains("mcp envref: desvio manual"), "{status}");
    let backups = count(&env.data.join("backups"));
    env.ok(&["apply", "--yes"]);
    let env_vars = &codex_servers(&env)["envref"]["env_vars"];
    assert_eq!(env_vars, &toml::Value::Array(vec!["E2E_FAKE_ENV".into()]));
    assert!(count(&env.data.join("backups")) > backups);

    let manual = "  manual:\n    command: node\n    args: [\"repo.js\"]\n";
    env.push(&yaml(Some("v2.js"), manual));
    assert!(env.ok(&["status"]).contains("mcp manual: conflito"));
    env.ok(&["apply", "--yes"]);
    assert_eq!(
        codex_servers(&env)["manual"]["args"],
        toml::Value::Array(vec!["mine".into()])
    );
    let adopted = env.ok(&["apply", "--yes", "--adopt"]);
    assert!(adopted.contains("assumir"), "{adopted}");
    assert_eq!(
        codex_servers(&env)["manual"]["args"],
        toml::Value::Array(vec!["repo.js".into()])
    );

    env.push(&yaml(None, manual));
    assert!(env.ok(&["status"]).contains("mcp plain: remover"));
    env.ok(&["apply", "--yes"]);
    assert_eq!(last_calls(&env, 1), vec![strs(&["mcp", "remove", "plain"])]);
    let s = codex_servers(&env);
    assert!(!s.contains_key("plain"));
    assert!(s.contains_key("untouched") && s.contains_key("envref"));
    assert!(codex_text(&env).starts_with("# user comment that must survive"));
    assert!(env.ok(&["status"]).contains("codex:\n  nada a mudar"));
}
