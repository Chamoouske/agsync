//! End to end with the real `opencode` CLI, every dir temporary.
//!
//! Before any `opencode mcp add`, the test proves the CLI writes to the temp config dir,
//! and it checks at the end that the real OpenCode config files did not change.
//! Skipped (with a message) when `opencode` is not in PATH.

mod common;

use std::path::PathBuf;
use std::process::Command;

use common::{Env, Untouched, count, fake_bin, git, path_with, text};
use jsonc_parser::ParseOptions;
use jsonc_parser::cst::CstRootNode;
use serde_json::Value;

const ENV_SECRET: &str = "oc-env-secret-111";
const FILE_SECRET: &str = "oc-file-secret-222";

const FIXTURE: &str = r#"{
  // user comment that must survive
  "$schema": "https://opencode.ai/config.json",
  "theme": "dark", /* inline comment */
  "mcp": {
    "legacy": { "type": "local", "command": ["legacy-tool"] },
    "servers": {
      "untouched": { "type": "local", "command": ["manual-tool", "keep"] },
    },
  },
}
"#;

fn real_files() -> Vec<PathBuf> {
    let mut dirs = vec![
        agsync_core::paths::home()
            .unwrap()
            .join(".config")
            .join("opencode"),
    ];
    for var in [
        "OPENCODE_CONFIG_DIR",
        "XDG_CONFIG_HOME",
        "ORCA_OPENCODE_CONFIG_DIR",
    ] {
        if let Some(dir) = std::env::var_os(var) {
            dirs.push(PathBuf::from(&dir));
            dirs.push(PathBuf::from(dir).join("opencode"));
        }
    }
    dirs.iter()
        .flat_map(|d| ["opencode.json", "opencode.jsonc"].map(|f| d.join(f)))
        .collect()
}

fn yaml(plain_arg: Option<&str>, extra: &str) -> String {
    let plain = match plain_arg {
        Some(arg) => format!("  plain:\n    command: node\n    args: [\"{arg}\"]\n"),
        None => String::new(),
    };
    format!(
        r#"version: 1
profiles: [work]
mcps:
{plain}  envref:
    command: node
    env: {{TOKEN: "${{E2E_OC_ENV}}"}}
  fileval:
    command: node
    env: {{TOKEN: "Bearer ${{E2E_OC_FILE}}"}}
  remote:
    url: https://example.com/mcp
    headers: {{Authorization: "Bearer ${{E2E_OC_ENV}}", X-Key: "${{E2E_OC_FILE}}"}}
{extra}skills:
  - path: skills/hello
  - path: skills/hello
    name: hello-codex
    agents: [codex]
"#
    )
}

fn opencode(env: &Env, args: &[&str]) -> String {
    let mut cmd = Command::new(agsync_core::which::find("opencode").unwrap());
    cmd.args(args);
    env.isolate(&mut cmd);
    cmd.env("OPENCODE_CONFIG_DIR", &env.opencode_dir);
    let out = cmd.output().unwrap();
    assert!(out.status.success(), "opencode {args:?}: {}", text(&out));
    text(&out)
}

fn config_text(env: &Env) -> String {
    std::fs::read_to_string(env.opencode_dir.join("opencode.jsonc")).unwrap()
}

fn servers(env: &Env) -> serde_json::Map<String, Value> {
    let root = CstRootNode::parse(&config_text(env), &ParseOptions::default()).unwrap();
    let root = root.to_serde_value().unwrap();
    root["mcp"]["servers"].as_object().cloned().unwrap()
}

#[test]
fn opencode_end_to_end() {
    if agsync_core::which::find("opencode").is_none() {
        eprintln!("SKIP opencode_end_to_end: `opencode` nao esta no PATH");
        return;
    }
    let real = Untouched::snapshot(&real_files());
    let mut env = Env::new();
    let bin = fake_bin(&env, &["codex"]);
    env.path = Some(path_with(&["git", "opencode", "sh"], &[bin]));
    env.vars = vec![("E2E_OC_ENV".to_string(), ENV_SECRET.to_string())];
    env.forbidden = vec![ENV_SECRET.to_string(), FILE_SECRET.to_string()];

    let added = opencode(&env, &["mcp", "add", "--global", "probe", "--", "probe"]);
    let probe_file = env.opencode_dir.join("opencode.json");
    assert!(added.contains(&probe_file.display().to_string()), "{added}");
    assert!(
        std::fs::read_to_string(&probe_file)
            .unwrap()
            .contains("probe")
    );
    real.assert_same();
    std::fs::remove_file(&probe_file).unwrap();

    std::fs::write(env.opencode_dir.join("opencode.jsonc"), FIXTURE).unwrap();
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
        &["secret", "set", "E2E_OC_FILE"],
        Some(&format!("{FILE_SECRET}\n")),
    );
    assert!(ok, "{out}");

    let status = env.ok(&["status"]);
    assert!(status.contains("opencode:\n"), "{status}");
    assert!(status.contains("mcp plain: adicionar"), "{status}");
    assert!(status.contains("skill hello: adicionar"), "{status}");
    let warning = "aviso: skill hello-codex fica visivel no opencode mesmo fora de agents: ele le as skills de codex";
    assert_eq!(status.matches(warning).count(), 1, "{status}");
    assert_eq!(env.ok(&["diff"]).matches(warning).count(), 1);
    assert!(
        status.contains("claude: claude nao encontrado no PATH"),
        "{status}"
    );

    let applied = env.ok(&["apply", "--yes"]);
    assert!(applied.contains("rode `opencode reload`"), "{applied}");
    let s = servers(&env);
    assert_eq!(s["plain"]["command"], serde_json::json!(["node", "v1.js"]));
    assert_eq!(s["envref"]["environment"]["TOKEN"], "{env:E2E_OC_ENV}");
    assert_eq!(
        s["fileval"]["environment"]["TOKEN"],
        format!("Bearer {FILE_SECRET}")
    );
    assert_eq!(s["remote"]["type"], "remote");
    assert_eq!(s["remote"]["url"], "https://example.com/mcp");
    let headers = &s["remote"]["headers"];
    assert_eq!(headers["Authorization"], "Bearer {env:E2E_OC_ENV}");
    assert_eq!(headers["X-Key"], FILE_SECRET);
    assert_eq!(
        s["untouched"]["command"],
        serde_json::json!(["manual-tool", "keep"])
    );
    let text = config_text(&env);
    assert!(text.contains("// user comment that must survive"), "{text}");
    assert!(text.contains("/* inline comment */"), "{text}");
    assert!(text.contains("\"theme\": \"dark\""), "{text}");
    assert!(text.contains("legacy-tool"), "{text}");
    assert!(!env.opencode_dir.join("opencode.json").exists());
    let linked = env
        .opencode_dir
        .join("skills")
        .join("hello")
        .join("SKILL.md");
    assert!(linked.is_file());
    let state = env.state();
    assert!(
        !state.contains(ENV_SECRET) && !state.contains(FILE_SECRET),
        "{state}"
    );
    assert!(env.ok(&["status"]).contains("opencode:\n  nada a mudar"));

    env.push(&yaml(Some("v2.js"), ""));
    assert!(env.ok(&["status"]).contains("mcp plain: atualizar"));
    env.ok(&["apply", "--yes"]);
    assert_eq!(
        servers(&env)["plain"]["command"],
        serde_json::json!(["node", "v2.js"])
    );

    let hacked = config_text(&env).replace("{env:E2E_OC_ENV}", "hacked");
    std::fs::write(env.opencode_dir.join("opencode.jsonc"), hacked).unwrap();
    let status = env.ok(&["status"]);
    assert!(status.contains("mcp envref: desvio manual"), "{status}");
    let backups = count(&env.data.join("backups"));
    env.ok(&["apply", "--yes"]);
    assert_eq!(
        servers(&env)["envref"]["environment"]["TOKEN"],
        "{env:E2E_OC_ENV}"
    );
    assert!(count(&env.data.join("backups")) > backups);

    let legacy = "  legacy:\n    command: node\n    args: [\"repo.js\"]\n";
    env.push(&yaml(Some("v2.js"), legacy));
    assert!(env.ok(&["status"]).contains("mcp legacy: conflito"));
    env.ok(&["apply", "--yes"]);
    assert!(config_text(&env).contains("legacy-tool"));
    let adopted = env.ok(&["apply", "--yes", "--adopt"]);
    assert!(adopted.contains("assumir"), "{adopted}");
    assert_eq!(
        servers(&env)["legacy"]["command"],
        serde_json::json!(["node", "repo.js"])
    );
    assert!(!config_text(&env).contains("legacy-tool"));

    env.push(&yaml(None, legacy));
    assert!(env.ok(&["status"]).contains("mcp plain: remover"));
    env.ok(&["apply", "--yes"]);
    let s = servers(&env);
    assert!(!s.contains_key("plain"));
    assert!(s.contains_key("untouched") && s.contains_key("envref"));
    assert!(config_text(&env).contains("// user comment that must survive"));
    assert!(env.ok(&["status"]).contains("opencode:\n  nada a mudar"));

    real.assert_same();
}
