//! Stand-in for the `codex` and `hermes` CLIs in tests, which never run the real ones.
//!
//! Tests copy this binary as `codex` or `hermes` into a temp dir put first in `PATH`;
//! the file name picks the agent. Each call appends its arguments as one JSON line to
//! `$FAKE_AGENT_LOG` (agent name first), then applies to the config file the effect the real CLI has.
//! The fake Hermes writes its YAML back as JSON (valid YAML): unlike the real one, it
//! does not keep comments.

use std::io::Write;
use std::path::PathBuf;

use toml_edit::{Array, DocumentMut, InlineTable, Item, Table};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let exe = std::env::current_exe().unwrap();
    let agent = exe.file_stem().unwrap().to_string_lossy().to_string();
    log(&agent, &args);
    let result = match agent.as_str() {
        "codex" => codex(&args),
        "hermes" => hermes(&args),
        other => Err(format!("agente falso desconhecido: {other}")),
    };
    if let Err(e) = result {
        eprintln!("{e}");
        std::process::exit(2);
    }
}

fn log(agent: &str, args: &[String]) {
    let Some(path) = std::env::var_os("FAKE_AGENT_LOG") else {
        return;
    };
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    let mut line = vec![agent.to_string()];
    line.extend_from_slice(args);
    writeln!(file, "{}", serde_json::to_string(&line).unwrap()).unwrap();
}

fn env_dir(var: &str) -> PathBuf {
    PathBuf::from(std::env::var_os(var).unwrap_or_else(|| panic!("{var} not set")))
}

fn codex(args: &[String]) -> Result<(), String> {
    let file = env_dir("CODEX_HOME").join("config.toml");
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    let mut doc: DocumentMut = text.parse().map_err(|e| format!("{e}"))?;
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["mcp", "add", name, rest @ ..] => {
            let entry = codex_entry(rest)?;
            if !doc.contains_key("mcp_servers") {
                let mut servers = Table::new();
                servers.set_implicit(true);
                doc.insert("mcp_servers", Item::Table(servers));
            }
            doc["mcp_servers"][*name] = Item::Table(entry);
        }
        ["mcp", "remove", name] => {
            if let Some(servers) = doc.get_mut("mcp_servers").and_then(Item::as_table_mut) {
                servers.remove(name);
            }
        }
        _ => return Err(format!("codex falso: comando nao suportado {args:?}")),
    }
    std::fs::write(&file, doc.to_string()).map_err(|e| e.to_string())
}

fn codex_entry(rest: &[&str]) -> Result<Table, String> {
    let mut table = Table::new();
    let mut env = InlineTable::new();
    let mut words = rest.iter();
    while let Some(word) = words.next() {
        match *word {
            "--env" => {
                let pair = words.next().ok_or("--env sem valor")?;
                let (key, value) = pair.split_once('=').ok_or("--env sem =")?;
                env.insert(key, value.into());
            }
            "--url" => {
                let url = words.next().ok_or("--url sem valor")?;
                table.insert("url", toml_edit::value(*url));
            }
            "--bearer-token-env-var" => {
                let var = words.next().ok_or("--bearer-token-env-var sem valor")?;
                table.insert("bearer_token_env_var", toml_edit::value(*var));
            }
            "--" => {
                let command = words.next().ok_or("comando ausente")?;
                table.insert("command", toml_edit::value(*command));
                let args: Array = words.by_ref().copied().collect();
                if !args.is_empty() {
                    table.insert("args", toml_edit::value(args));
                }
            }
            other => return Err(format!("flag desconhecida {other}")),
        }
    }
    if !env.is_empty() {
        table.insert("env", toml_edit::value(env));
    }
    Ok(table)
}

fn hermes(args: &[String]) -> Result<(), String> {
    let file = env_dir("HERMES_HOME").join("config.yaml");
    let text = std::fs::read_to_string(&file).unwrap_or_default();
    let mut root: serde_json::Value = if text.trim().is_empty() {
        serde_json::json!({})
    } else {
        serde_saphyr::from_str(&text).map_err(|e| e.to_string())?
    };
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["config", "set", path, value] => {
            let (top, name) = path.split_once('.').ok_or("caminho sem ponto")?;
            let value = serde_json::from_str(value)
                .unwrap_or_else(|_| serde_json::Value::String(value.to_string()));
            let map = root.as_object_mut().ok_or("raiz nao e mapa")?;
            let section = map.entry(top).or_insert_with(|| serde_json::json!({}));
            section[name] = value;
        }
        ["config", "unset", path] => {
            let (top, name) = path.split_once('.').ok_or("caminho sem ponto")?;
            if let Some(section) = root.get_mut(top).and_then(|s| s.as_object_mut()) {
                section.remove(name);
            }
        }
        _ => return Err(format!("hermes falso: comando nao suportado {args:?}")),
    }
    let yaml = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    std::fs::write(&file, yaml).map_err(|e| e.to_string())
}
