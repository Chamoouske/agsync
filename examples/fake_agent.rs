//! Stand-in for the `codex`, `hermes` and `agy` CLIs in tests, which never run the real
//! ones.
//!
//! Tests copy this binary under one of those names into a temp dir put first in `PATH`;
//! the file name picks the agent. Each call appends its arguments as one JSON line to
//! `$FAKE_AGENT_LOG` (agent name first), then applies to the config file the effect the real CLI has.
//! The fake Hermes writes its YAML back as JSON (valid YAML): unlike the real one, it
//! does not keep comments.
//! The fake `codex plugin` reads catalogs from `.claude-plugin/marketplace.json`, clones
//! git marketplaces with `git`, and records each installed plugin with its version in
//! `[plugins."<plugin>@<marketplace>"]`.

use std::io::Write;
use std::path::{Path, PathBuf};

use toml_edit::{Array, DocumentMut, InlineTable, Item, Table};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let exe = std::env::current_exe().unwrap();
    let agent = exe.file_stem().unwrap().to_string_lossy().to_string();
    log(&agent, &args);
    let result = match agent.as_str() {
        "codex" => codex(&args),
        "hermes" => hermes(&args),
        "agy" => agy(&args),
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
        ["plugin", rest @ ..] => {
            if !codex_plugin(rest, &mut doc)? {
                return Ok(());
            }
        }
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

/// Runs one `codex plugin` call; returns true when `config.toml` changed.
fn codex_plugin(words: &[&str], doc: &mut DocumentMut) -> Result<bool, String> {
    match words {
        ["marketplace", "add", source, "--ref", git_ref, "--json"] => {
            marketplace_add(doc, source, Some(git_ref))?
        }
        ["marketplace", "add", source, "--json"] => marketplace_add(doc, source, None)?,
        ["marketplace", "remove", name, "--json"] => marketplace_remove(doc, name)?,
        ["add", id, "--json"] => plugin_add(doc, id)?,
        ["remove", id, "--json"] => {
            let removed = table_mut(doc, "plugins").remove(id);
            removed.ok_or(format!("plugin `{id}` is not installed"))?;
        }
        ["list", "--json"] => {
            plugin_list(doc);
            return Ok(false);
        }
        _ => return Err(format!("codex falso: comando nao suportado {words:?}")),
    }
    Ok(true)
}

fn table_mut<'a>(doc: &'a mut DocumentMut, key: &str) -> &'a mut Table {
    if !doc.contains_key(key) {
        let mut table = Table::new();
        table.set_implicit(true);
        doc.insert(key, Item::Table(table));
    }
    doc[key].as_table_mut().unwrap()
}

fn marketplaces_dir() -> PathBuf {
    env_dir("CODEX_HOME").join(".tmp").join("marketplaces")
}

/// Name and `(plugin, version)` pairs of the catalog under `root`.
fn catalog(root: &Path) -> Result<(String, Vec<(String, String)>), String> {
    let file = root.join(".claude-plugin").join("marketplace.json");
    let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let name = value["name"]
        .as_str()
        .ok_or("catalogo sem name")?
        .to_string();
    let plugins = value["plugins"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| {
            let field = |key: &str| p[key].as_str().unwrap_or_default().to_string();
            (field("name"), field("version"))
        })
        .collect();
    Ok((name, plugins))
}

fn clone(source: &str, git_ref: Option<&str>, dest: &Path) -> Result<(), String> {
    let _ = std::fs::remove_dir_all(dest);
    let mut cmd = std::process::Command::new("git");
    cmd.args(["clone", "-q"]);
    if let Some(git_ref) = git_ref {
        cmd.args(["--branch", git_ref]);
    }
    let status = cmd
        .arg(source)
        .arg(dest)
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("failed to clone marketplace {source}"));
    }
    Ok(())
}

fn marketplace_add(
    doc: &mut DocumentMut,
    source: &str,
    git_ref: Option<&str>,
) -> Result<(), String> {
    let local = Path::new(source).is_dir();
    let staging = env_dir("CODEX_HOME").join(".tmp").join("staging");
    let root = if local {
        PathBuf::from(source)
    } else {
        clone(source, git_ref, &staging)?;
        staging.clone()
    };
    let (name, _) = catalog(&root)?;
    let existing = doc.get("marketplaces").and_then(|m| m.get(&name));
    let already = existing.is_some();
    if let Some(entry) = existing {
        let same = entry.get("source").and_then(Item::as_str) == Some(source)
            && entry.get("ref").and_then(Item::as_str) == git_ref;
        let _ = std::fs::remove_dir_all(&staging);
        if !same {
            return Err(format!(
                "marketplace '{name}' is already added from a different source; remove it before adding this source"
            ));
        }
    } else {
        if !local {
            std::fs::create_dir_all(marketplaces_dir()).map_err(|e| e.to_string())?;
            std::fs::rename(&staging, marketplaces_dir().join(&name)).map_err(|e| e.to_string())?;
        }
        let mut entry = Table::new();
        entry.insert(
            "source_type",
            toml_edit::value(if local { "local" } else { "git" }),
        );
        entry.insert("source", toml_edit::value(source));
        if let Some(git_ref) = git_ref {
            entry.insert("ref", toml_edit::value(git_ref));
        }
        table_mut(doc, "marketplaces").insert(&name, Item::Table(entry));
    }
    let out =
        serde_json::json!({"marketplaceName": name, "installedRoot": "", "alreadyAdded": already});
    println!("{out}");
    Ok(())
}

fn marketplace_remove(doc: &mut DocumentMut, name: &str) -> Result<(), String> {
    table_mut(doc, "marketplaces").remove(name).ok_or(format!(
        "marketplace `{name}` is not configured or installed"
    ))?;
    let _ = std::fs::remove_dir_all(marketplaces_dir().join(name));
    println!("{}", serde_json::json!({"marketplaceName": name}));
    Ok(())
}

fn plugin_add(doc: &mut DocumentMut, id: &str) -> Result<(), String> {
    let (plugin, market) = id.split_once('@').ok_or("esperado PLUGIN@MARKETPLACE")?;
    let entry = doc
        .get("marketplaces")
        .and_then(|m| m.get(market))
        .ok_or(format!("marketplace `{market}` not found"))?;
    let root = match entry.get("source_type").and_then(Item::as_str) {
        Some("local") => PathBuf::from(entry["source"].as_str().unwrap_or_default()),
        _ => marketplaces_dir().join(market),
    };
    let (_, plugins) = catalog(&root)?;
    let (_, version) = plugins
        .into_iter()
        .find(|(name, _)| name == plugin)
        .ok_or(format!(
            "plugin `{plugin}` was not found in marketplace `{market}`"
        ))?;
    let mut installed = Table::new();
    installed.insert("enabled", toml_edit::value(true));
    installed.insert("version", toml_edit::value(version.as_str()));
    table_mut(doc, "plugins").insert(id, Item::Table(installed));
    let out = serde_json::json!({"pluginId": id, "name": plugin, "marketplaceName": market, "version": version});
    println!("{out}");
    Ok(())
}

fn plugin_list(doc: &DocumentMut) {
    let installed: Vec<serde_json::Value> = doc
        .get("plugins")
        .and_then(Item::as_table)
        .into_iter()
        .flat_map(|t| t.iter())
        .map(|(id, entry)| {
            let (name, market) = id.split_once('@').unwrap_or((id, ""));
            let version = entry.get("version").and_then(Item::as_str).unwrap_or_default();
            serde_json::json!({"pluginId": id, "name": name, "marketplaceName": market, "version": version, "installed": true, "enabled": true})
        })
        .collect();
    println!(
        "{}",
        serde_json::json!({"installed": installed, "available": []})
    );
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

/// `agy plugin`: installs copy `plugin.json` into `~/.gemini/config/plugins/<name>`.
fn agy(args: &[String]) -> Result<(), String> {
    let home = env_dir(if cfg!(windows) { "USERPROFILE" } else { "HOME" });
    let plugins = home.join(".gemini").join("config").join("plugins");
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["plugin", "--help"] => Ok(()),
        ["plugin", "install", source] => {
            let manifest = PathBuf::from(source).join("plugin.json");
            let dir = plugins.join(manifest_name(&manifest)?);
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            std::fs::copy(&manifest, dir.join("plugin.json"))
                .map(|_| ())
                .map_err(|e| e.to_string())
        }
        ["plugin", "uninstall", name] => {
            std::fs::remove_dir_all(plugins.join(name)).map_err(|e| e.to_string())
        }
        _ => Err(format!("agy falso: comando nao suportado {args:?}")),
    }
}

fn manifest_name(file: &std::path::Path) -> Result<String, String> {
    let text = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let name = value["name"].as_str().ok_or("manifesto sem name")?;
    Ok(name.to_string())
}
