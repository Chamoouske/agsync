//! A JSON config file, comments allowed, that keeps MCP servers in one top-level object.
//!
//! Edits go through the JSONC syntax tree, so comments, formatting and every other key
//! stay as they were. Used by agents that have no non-interactive MCP command.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::Context;
use jsonc_parser::ParseOptions;
use jsonc_parser::cst::{CstInputValue, CstRootNode};
use serde_json::{Map, Value, json};

use crate::hash::hash_bytes;
use crate::mcp::McpSpec;

/// One config file and the key that holds its MCP servers.
#[derive(Debug)]
pub struct JsonConfig {
    pub file: PathBuf,
    key: &'static str,
}

impl JsonConfig {
    pub fn new(file: PathBuf, key: &'static str) -> JsonConfig {
        JsonConfig { file, key }
    }

    /// Every entry under the key; empty when the file or the key is missing.
    pub fn servers(&self) -> anyhow::Result<Map<String, Value>> {
        let Some(root) = self.parse()? else {
            return Ok(Map::new());
        };
        let servers = root
            .to_serde_value()
            .and_then(|v| v.get(self.key).and_then(Value::as_object).cloned());
        Ok(servers.unwrap_or_default())
    }

    /// Creates or replaces the entry `name`.
    pub fn set(&self, name: &str, entry: &Value) -> anyhow::Result<()> {
        let root = match self.parse()? {
            Some(root) => root,
            None => parse("{}\n").context("JSON vazio")?,
        };
        let servers = root.object_value_or_set().object_value_or_set(self.key);
        let value = input(entry);
        match servers.get(name) {
            Some(prop) => prop.set_value(value),
            None => {
                servers.append(name, value);
            }
        }
        crate::atomic::write_atomic(&self.file, root.to_string().as_bytes())
    }

    /// Removes the entry `name`; a missing file or entry is left alone.
    pub fn remove(&self, name: &str) -> anyhow::Result<()> {
        let Some(root) = self.parse()? else {
            return Ok(());
        };
        let servers = root.object_value().and_then(|o| o.object_value(self.key));
        let Some(prop) = servers.and_then(|s| s.get(name)) else {
            return Ok(());
        };
        prop.remove();
        crate::atomic::write_atomic(&self.file, root.to_string().as_bytes())
    }

    /// The syntax tree; `None` when the file is missing or blank.
    fn parse(&self) -> anyhow::Result<Option<CstRootNode>> {
        let text = match std::fs::read_to_string(&self.file) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e).with_context(|| format!("ler {}", self.file.display())),
        };
        if text.trim().is_empty() {
            return Ok(None);
        }
        parse(&text)
            .map(Some)
            .with_context(|| format!("{} invalido", self.file.display()))
    }
}

/// JSON file as a value; `None` when it is missing.
pub fn read(file: &Path) -> anyhow::Result<Option<Value>> {
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("ler {}", file.display())),
    };
    let value =
        serde_json::from_str(&text).with_context(|| format!("{} invalido", file.display()))?;
    Ok(Some(value))
}

/// The entry in the common `mcpServers` shape: `command`, `args` and `env`, or the
/// remote URL under `url_key` with `headers`. Empty lists and maps are left out.
pub fn entry(spec: &McpSpec, url_key: &str) -> Value {
    let mut entry = Map::new();
    match spec {
        McpSpec::Stdio { command, args, env } => {
            entry.insert("command".into(), json!(command));
            if !args.is_empty() {
                entry.insert("args".into(), json!(args));
            }
            if !env.is_empty() {
                entry.insert("env".into(), json!(env));
            }
        }
        McpSpec::Http { url, headers } => {
            entry.insert(url_key.into(), json!(url));
            if !headers.is_empty() {
                entry.insert("headers".into(), json!(headers));
            }
        }
    }
    Value::Object(entry)
}

/// The neutral form of an entry written by [`entry`]. Other keys (`trust`, `timeout`,
/// `disabled`...) are ignored; `None` when the shape is not one agsync writes.
pub fn spec_of(value: &Value, url_key: &str) -> Option<McpSpec> {
    if let Some(url) = value.get(url_key) {
        let url = url.as_str()?.to_string();
        let headers = string_map(value.get("headers"))?;
        return Some(McpSpec::Http { url, headers });
    }
    let command = value.get("command")?.as_str()?.to_string();
    let args = match value.get("args") {
        Some(args) => args
            .as_array()?
            .iter()
            .map(|a| a.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()?,
        None => Vec::new(),
    };
    let env = string_map(value.get("env"))?;
    Some(McpSpec::Stdio { command, args, env })
}

/// Hash of an entry: its neutral form when it has one, else its raw JSON.
pub fn entry_hash(value: &Value, spec: Option<McpSpec>) -> String {
    match spec {
        Some(spec) => spec.hash(),
        None => hash_bytes(value.to_string().as_bytes()),
    }
}

fn string_map(value: Option<&Value>) -> Option<BTreeMap<String, String>> {
    let Some(value) = value else {
        return Some(BTreeMap::new());
    };
    value
        .as_object()?
        .iter()
        .map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_string())))
        .collect()
}

fn parse(text: &str) -> anyhow::Result<CstRootNode> {
    CstRootNode::parse(text, &ParseOptions::default()).map_err(|e| anyhow::anyhow!("{e}"))
}

fn input(value: &Value) -> CstInputValue {
    match value {
        Value::Null => CstInputValue::Null,
        Value::Bool(b) => CstInputValue::Bool(*b),
        Value::Number(n) => CstInputValue::Number(n.to_string()),
        Value::String(s) => CstInputValue::String(s.clone()),
        Value::Array(items) => CstInputValue::Array(items.iter().map(input).collect()),
        Value::Object(map) => {
            CstInputValue::Object(map.iter().map(|(k, v)| (k.clone(), input(v))).collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmp::TempDir;

    #[test]
    fn entries_round_trip_with_the_url_key() {
        let stdio = McpSpec::Stdio {
            command: "tool".into(),
            args: vec!["a".into()],
            env: BTreeMap::from([("T".to_string(), "${T}".to_string())]),
        };
        let written = entry(&stdio, "httpUrl");
        assert_eq!(
            written.to_string(),
            r#"{"args":["a"],"command":"tool","env":{"T":"${T}"}}"#
        );
        assert_eq!(spec_of(&written, "httpUrl"), Some(stdio));

        let http = McpSpec::Http {
            url: "https://x/mcp".into(),
            headers: BTreeMap::new(),
        };
        let written = entry(&http, "serverUrl");
        assert_eq!(written, json!({"serverUrl": "https://x/mcp"}));
        assert_eq!(spec_of(&written, "serverUrl"), Some(http));
        assert_eq!(spec_of(&json!({"command": 1}), "url"), None);
    }

    const FIXTURE: &str = r#"{
  // user comment
  "theme": "dark", /* inline */
  "mcpServers": {
    "manual": { "command": "tool", "args": ["a"] },
  },
}
"#;

    #[test]
    fn set_and_remove_keep_comments_and_other_entries() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let config = JsonConfig::new(tmp.path().join("settings.json"), "mcpServers");
        std::fs::write(&config.file, FIXTURE).unwrap();

        config
            .set("new", &json!({"command": "node", "args": ["x"]}))
            .unwrap();
        config.set("manual", &json!({"command": "other"})).unwrap();
        let text = std::fs::read_to_string(&config.file).unwrap();
        assert!(text.contains("// user comment"), "{text}");
        assert!(text.contains("/* inline */"), "{text}");
        let servers = config.servers().unwrap();
        assert_eq!(servers["new"], json!({"command": "node", "args": ["x"]}));
        assert_eq!(servers["manual"], json!({"command": "other"}));

        config.remove("new").unwrap();
        config.remove("never-there").unwrap();
        let text = std::fs::read_to_string(&config.file).unwrap();
        assert!(text.contains("\"theme\": \"dark\""), "{text}");
        assert_eq!(
            config.servers().unwrap().keys().collect::<Vec<_>>(),
            ["manual"]
        );
    }

    #[test]
    fn creates_a_missing_or_blank_file() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let config = JsonConfig::new(tmp.path().join("sub").join("mcp.json"), "mcpServers");
        assert!(config.servers().unwrap().is_empty());
        config.remove("x").unwrap();
        assert!(!config.file.exists());
        config.set("x", &json!({"serverUrl": "https://x"})).unwrap();
        assert_eq!(
            config.servers().unwrap()["x"],
            json!({"serverUrl": "https://x"})
        );

        std::fs::write(&config.file, "  \n").unwrap();
        assert!(config.servers().unwrap().is_empty());
        config.set("y", &json!({"command": "c"})).unwrap();
        assert_eq!(config.servers().unwrap().len(), 1);
    }
}
