//! Hermes Agent adapter.
//!
//! `HERMES_HOME` is the env var, else `%LOCALAPPDATA%\hermes` on Windows and `~/.hermes`
//! elsewhere; every `hermes` call gets it explicitly. MCP servers live in
//! `<HERMES_HOME>/config.yaml`, top-level `mcp_servers`.
//!
//! `hermes mcp add/remove` are interactive, so writes use the non-interactive
//! `hermes config set mcp_servers.<name> '<json>'` and `hermes config unset`; Hermes
//! itself round-trips the YAML (comments kept) and writes it atomically.
//! Skills are folders under `<HERMES_HOME>/skills`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;
use serde_json::{Value, json};

use super::{Adapter, backup, cli, link};
use crate::deliver::RefStyle;
use crate::hash::hash_bytes;
use crate::mcp::McpSpec;
use crate::state::LinkMode;

/// Hermes Agent home.
#[derive(Debug)]
pub struct Hermes {
    exe: PathBuf,
    home: PathBuf,
}

impl Hermes {
    /// Finds `hermes` in PATH and resolves `HERMES_HOME`.
    pub fn detect() -> Result<Hermes, String> {
        let exe = crate::which::find("hermes").ok_or("hermes nao encontrado no PATH")?;
        let home = home_dir().map_err(|e| e.to_string())?;
        Ok(Hermes::at(exe, home))
    }

    /// Adapter for an explicit `HERMES_HOME`.
    pub fn at(exe: PathBuf, home: PathBuf) -> Hermes {
        Hermes { exe, home }
    }

    fn config_file(&self) -> PathBuf {
        self.home.join("config.yaml")
    }

    fn servers(&self) -> anyhow::Result<serde_json::Map<String, Value>> {
        let file = self.config_file();
        let text = match std::fs::read_to_string(&file) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Default::default()),
            Err(e) => return Err(e).with_context(|| format!("ler {}", file.display())),
        };
        if text.trim().is_empty() {
            return Ok(Default::default());
        }
        let root: Value = serde_saphyr::from_str(&text)
            .map_err(|e| anyhow::anyhow!("{} invalido: {e}", file.display()))?;
        let servers = root.get("mcp_servers").and_then(Value::as_object);
        Ok(servers.cloned().unwrap_or_default())
    }

    fn written_hash(&self, name: &str) -> anyhow::Result<Option<String>> {
        Ok(self.servers()?.get(name).map(entry_hash))
    }

    fn run(&self, args: &[&str], show_output: bool) -> anyhow::Result<()> {
        let mut cmd = Command::new(&self.exe);
        cmd.env("HERMES_HOME", &self.home).args(args);
        let what = format!("hermes {}", args[..2].join(" "));
        cli::run(cmd, &what, show_output)
    }
}

impl Adapter for Hermes {
    fn ref_style(&self) -> RefStyle {
        RefStyle::Anywhere
    }

    fn mcps(&self) -> anyhow::Result<BTreeMap<String, String>> {
        let servers = self.servers()?;
        Ok(servers
            .iter()
            .map(|(name, value)| (name.clone(), entry_hash(value)))
            .collect())
    }

    fn skills(&self) -> anyhow::Result<BTreeMap<String, String>> {
        link::installed(&self.home.join("skills"))
    }

    fn backup(&self, dir: &Path) -> anyhow::Result<()> {
        backup::copy(&self.config_file(), dir, "hermes-config.yaml")
    }

    fn write_mcp(&self, name: &str, spec: &McpSpec) -> anyhow::Result<()> {
        let key = format!("mcp_servers.{name}");
        if self.written_hash(name)?.is_some() {
            self.run(&["config", "unset", &key], true)?;
        }
        let entry = native(spec)?.to_string();
        self.run(&["config", "set", &key, &entry], false)?;
        if self.written_hash(name)?.as_deref() != Some(spec.hash().as_str()) {
            anyhow::bail!("hermes nao gravou o mcp '{name}' como esperado");
        }
        Ok(())
    }

    fn remove_mcp(&self, name: &str) -> anyhow::Result<()> {
        if self.written_hash(name)?.is_none() {
            return Ok(());
        }
        self.run(&["config", "unset", &format!("mcp_servers.{name}")], true)?;
        if self.written_hash(name)?.is_some() {
            anyhow::bail!("mcp '{name}' continua em {}", self.config_file().display());
        }
        Ok(())
    }

    fn write_skill(&self, name: &str, cached: &Path, backups: &Path) -> anyhow::Result<LinkMode> {
        link::install(cached, &self.home.join("skills").join(name), backups)
    }

    fn remove_skill(&self, name: &str, backups: &Path) -> anyhow::Result<()> {
        link::clear(&self.home.join("skills").join(name), backups)
    }
}

/// `HERMES_HOME`, else the platform default of native Hermes.
pub fn home_dir() -> anyhow::Result<PathBuf> {
    if let Some(dir) = std::env::var_os("HERMES_HOME") {
        return Ok(PathBuf::from(dir));
    }
    if cfg!(windows) {
        let local =
            std::env::var_os("LOCALAPPDATA").context("variavel LOCALAPPDATA nao definida")?;
        return Ok(PathBuf::from(local).join("hermes"));
    }
    Ok(crate::paths::home()?.join(".hermes"))
}

/// The entry Hermes stores, as JSON for `hermes config set`.
fn native(spec: &McpSpec) -> anyhow::Result<Value> {
    let McpSpec::Stdio { command, args, env } = spec else {
        anyhow::bail!("mcp remoto ainda nao suportado no hermes");
    };
    let mut entry = json!({ "command": command });
    if !args.is_empty() {
        entry["args"] = json!(args);
    }
    if !env.is_empty() {
        entry["env"] = json!(env);
    }
    Ok(entry)
}

fn entry_hash(value: &Value) -> String {
    match to_spec(value) {
        Some(spec) => spec.hash(),
        None => hash_bytes(value.to_string().as_bytes()),
    }
}

/// The neutral form of a Hermes entry; other keys such as `enabled` are ignored.
fn to_spec(value: &Value) -> Option<McpSpec> {
    let command = value.get("command")?.as_str()?.to_string();
    let args = match value.get("args") {
        Some(args) => args
            .as_array()?
            .iter()
            .map(|a| a.as_str().map(str::to_string))
            .collect::<Option<Vec<_>>>()?,
        None => Vec::new(),
    };
    let mut env = BTreeMap::new();
    for (key, value) in value
        .get("env")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
    {
        env.insert(key.clone(), value.as_str()?.to_string());
    }
    Some(McpSpec::Stdio { command, args, env })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmp::TempDir;

    const CONFIG: &str = "# user comment\nmodel: x\nmcp_servers:\n  manual:\n    command: tool\n    args: [a]\n    env:\n      TOKEN: ${TOKEN}\n    enabled: true\n";

    #[test]
    fn reads_yaml_entries_and_builds_json_for_config_set() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let hermes = Hermes::at("hermes-not-called".into(), tmp.path().into());
        assert!(hermes.mcps().unwrap().is_empty());
        std::fs::write(tmp.path().join("config.yaml"), CONFIG).unwrap();
        let spec = McpSpec::Stdio {
            command: "tool".into(),
            args: vec!["a".into()],
            env: BTreeMap::from([("TOKEN".to_string(), "${TOKEN}".to_string())]),
        };
        assert_eq!(hermes.mcps().unwrap()["manual"], spec.hash());
        let entry = native(&spec).unwrap();
        assert_eq!(
            entry.to_string(),
            r#"{"args":["a"],"command":"tool","env":{"TOKEN":"${TOKEN}"}}"#
        );
        assert_eq!(to_spec(&entry), Some(spec));
    }
}
