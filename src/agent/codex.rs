//! Codex CLI adapter.
//!
//! MCP servers live in `$CODEX_HOME/config.toml` (default `~/.codex`), table
//! `[mcp_servers.<name>]`. Every `codex` call gets `CODEX_HOME` explicitly.
//!
//! `codex mcp add/remove` is used whenever it can express the entry. It has no flag for
//! `env_vars`, so an entry that passes a variable through from the environment is
//! written by editing the TOML in place (`toml_edit`, comments and layout kept).
//! Skills are folders under `~/.agents/skills`, the user skills root Codex reads.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;
use toml_edit::{Array, DocumentMut, InlineTable, Item, Table};

use super::{Adapter, backup, cli, link};
use crate::deliver::RefStyle;
use crate::hash::hash_bytes;
use crate::mcp::McpSpec;
use crate::secret::whole_ref;
use crate::state::LinkMode;

/// Codex user config.
#[derive(Debug)]
pub struct Codex {
    exe: PathBuf,
    home: PathBuf,
    skills_dir: PathBuf,
}

impl Codex {
    /// Finds `codex` in PATH and resolves `CODEX_HOME` and the skills root.
    pub fn detect() -> Result<Codex, String> {
        let exe = crate::which::find("codex").ok_or("codex nao encontrado no PATH")?;
        let user_home = crate::paths::home().map_err(|e| e.to_string())?;
        let home = match std::env::var_os("CODEX_HOME") {
            Some(dir) => PathBuf::from(dir),
            None => user_home.join(".codex"),
        };
        Ok(Codex::at(
            exe,
            home,
            user_home.join(".agents").join("skills"),
        ))
    }

    /// Adapter for explicit locations.
    pub fn at(exe: PathBuf, home: PathBuf, skills_dir: PathBuf) -> Codex {
        Codex {
            exe,
            home,
            skills_dir,
        }
    }

    fn config_file(&self) -> PathBuf {
        self.home.join("config.toml")
    }

    fn read_text(&self) -> anyhow::Result<String> {
        let file = self.config_file();
        match std::fs::read_to_string(&file) {
            Ok(text) => Ok(text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(e).with_context(|| format!("ler {}", file.display())),
        }
    }

    fn servers(&self) -> anyhow::Result<toml::Table> {
        let table: toml::Table = toml::from_str(&self.read_text()?)
            .with_context(|| format!("{} invalido", self.config_file().display()))?;
        let servers = table.get("mcp_servers").and_then(|v| v.as_table());
        Ok(servers.cloned().unwrap_or_default())
    }

    fn written_hash(&self, name: &str) -> anyhow::Result<Option<String>> {
        Ok(self.servers()?.get(name).map(entry_hash))
    }

    fn run(&self, args: &[String], show_output: bool) -> anyhow::Result<()> {
        let mut cmd = Command::new(&self.exe);
        cmd.env("CODEX_HOME", &self.home).args(args);
        let what = format!("codex {}", args[..2].join(" "));
        cli::run(cmd, &what, show_output)
    }

    /// Sets `[mcp_servers.<name>]` to `entry`, keeping the rest of the file.
    fn set_entry(&self, name: &str, entry: Table) -> anyhow::Result<()> {
        let file = self.config_file();
        let mut doc: DocumentMut = self
            .read_text()?
            .parse()
            .with_context(|| format!("{} invalido", file.display()))?;
        if !doc.contains_key("mcp_servers") {
            let mut servers = Table::new();
            servers.set_implicit(true);
            doc.insert("mcp_servers", Item::Table(servers));
        }
        let servers = doc["mcp_servers"]
            .as_table_mut()
            .with_context(|| format!("mcp_servers em {} nao e uma tabela", file.display()))?;
        servers.insert(name, Item::Table(entry));
        crate::atomic::write_atomic(&file, doc.to_string().as_bytes())
    }
}

impl Adapter for Codex {
    fn ref_style(&self) -> RefStyle {
        RefStyle::Codex
    }

    fn mcps(&self) -> anyhow::Result<BTreeMap<String, String>> {
        let servers = self.servers()?;
        Ok(servers
            .iter()
            .map(|(name, value)| (name.clone(), entry_hash(value)))
            .collect())
    }

    fn skills(&self) -> anyhow::Result<BTreeMap<String, String>> {
        link::installed(&self.skills_dir)
    }

    fn backup(&self, dir: &Path) -> anyhow::Result<()> {
        backup::copy(&self.config_file(), dir, "codex-config.toml")
    }

    fn write_mcp(&self, name: &str, spec: &McpSpec) -> anyhow::Result<()> {
        match cli_args(name, spec)? {
            Some(args) => {
                if self.written_hash(name)?.is_some() {
                    self.run(&["mcp".into(), "remove".into(), name.into()], true)?;
                }
                self.run(&args, false)?;
            }
            None => self.set_entry(name, native(spec)?)?,
        }
        if self.written_hash(name)?.as_deref() != Some(spec.hash().as_str()) {
            anyhow::bail!("codex nao gravou o mcp '{name}' como esperado");
        }
        Ok(())
    }

    fn remove_mcp(&self, name: &str) -> anyhow::Result<()> {
        if self.written_hash(name)?.is_none() {
            return Ok(());
        }
        self.run(&["mcp".into(), "remove".into(), name.into()], true)?;
        if self.written_hash(name)?.is_some() {
            anyhow::bail!("mcp '{name}' continua em {}", self.config_file().display());
        }
        Ok(())
    }

    fn write_skill(&self, name: &str, cached: &Path, backups: &Path) -> anyhow::Result<LinkMode> {
        link::install(cached, &self.skills_dir.join(name), backups)
    }

    fn remove_skill(&self, name: &str, backups: &Path) -> anyhow::Result<()> {
        link::clear(&self.skills_dir.join(name), backups)
    }
}

/// Env keys whose value is exactly `${KEY}`: passed through from Codex's environment.
fn passed_through(env: &BTreeMap<String, String>) -> Vec<&str> {
    env.iter()
        .filter(|(key, value)| whole_ref(value) == Some(key.as_str()))
        .map(|(key, _)| key.as_str())
        .collect()
}

/// `codex mcp add` arguments, or `None` when the CLI has no flag for some field.
fn cli_args(name: &str, spec: &McpSpec) -> anyhow::Result<Option<Vec<String>>> {
    let McpSpec::Stdio { command, args, env } = spec else {
        anyhow::bail!("mcp remoto ainda nao suportado no codex");
    };
    if !passed_through(env).is_empty() {
        return Ok(None);
    }
    let mut out: Vec<String> = vec!["mcp".into(), "add".into(), name.into()];
    for (key, value) in env {
        out.push("--env".into());
        out.push(format!("{key}={value}"));
    }
    out.push("--".into());
    out.push(command.clone());
    out.extend(args.iter().cloned());
    Ok(Some(out))
}

/// The Codex table for an entry.
fn native(spec: &McpSpec) -> anyhow::Result<Table> {
    let McpSpec::Stdio { command, args, env } = spec else {
        anyhow::bail!("mcp remoto ainda nao suportado no codex");
    };
    let mut table = Table::new();
    table.insert("command", toml_edit::value(command.as_str()));
    if !args.is_empty() {
        table.insert("args", toml_edit::value(Array::from_iter(args)));
    }
    let names = passed_through(env);
    let fixed: InlineTable = env
        .iter()
        .filter(|(key, _)| !names.contains(&key.as_str()))
        .map(|(key, value)| (key.as_str(), toml_edit::Value::from(value.as_str())))
        .collect();
    if !fixed.is_empty() {
        table.insert("env", toml_edit::value(fixed));
    }
    if !names.is_empty() {
        table.insert("env_vars", toml_edit::value(Array::from_iter(names)));
    }
    Ok(table)
}

fn entry_hash(value: &toml::Value) -> String {
    match to_spec(value) {
        Some(spec) => spec.hash(),
        None => hash_bytes(serde_json::to_string(value).unwrap_or_default().as_bytes()),
    }
}

/// The neutral form of a Codex entry: `env_vars` names read back as `NAME: ${NAME}`.
fn to_spec(value: &toml::Value) -> Option<McpSpec> {
    let table = value.as_table()?;
    let command = table.get("command")?.as_str()?.to_string();
    let args = match table.get("args") {
        Some(args) => strings(args)?,
        None => Vec::new(),
    };
    let mut env = BTreeMap::new();
    for (key, value) in table
        .get("env")
        .and_then(|e| e.as_table())
        .into_iter()
        .flatten()
    {
        env.insert(key.clone(), value.as_str()?.to_string());
    }
    if let Some(names) = table.get("env_vars") {
        for name in strings(names)? {
            env.insert(name.clone(), format!("${{{name}}}"));
        }
    }
    Some(McpSpec::Stdio { command, args, env })
}

fn strings(value: &toml::Value) -> Option<Vec<String>> {
    value
        .as_array()?
        .iter()
        .map(|v| v.as_str().map(str::to_string))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmp::TempDir;

    const CONFIG: &str = r#"# user comment
model = "o3"

[profiles.fast]
model = "o4-mini" # trailing

[mcp_servers.manual]
command = "tool"
args = ["a"]
env_vars = ["TOKEN"]
env = { MODE = "x" }
"#;

    fn stdio(env: &[(&str, &str)]) -> McpSpec {
        McpSpec::Stdio {
            command: "node".into(),
            args: vec!["srv.js".into()],
            env: env
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    #[test]
    fn reads_env_vars_as_references() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let codex = Codex::at(
            "codex-not-called".into(),
            tmp.path().into(),
            tmp.path().into(),
        );
        assert!(codex.mcps().unwrap().is_empty());
        std::fs::write(tmp.path().join("config.toml"), CONFIG).unwrap();
        let expected = McpSpec::Stdio {
            command: "tool".into(),
            args: vec!["a".into()],
            env: BTreeMap::from([
                ("MODE".to_string(), "x".to_string()),
                ("TOKEN".to_string(), "${TOKEN}".to_string()),
            ]),
        };
        assert_eq!(codex.mcps().unwrap()["manual"], expected.hash());
    }

    #[test]
    fn cli_only_when_no_env_var_is_passed_through() {
        let plain = stdio(&[("MODE", "x"), ("TOKEN", "Bearer value")]);
        let args = cli_args("srv", &plain).unwrap().unwrap();
        assert_eq!(
            args,
            [
                "mcp",
                "add",
                "srv",
                "--env",
                "MODE=x",
                "--env",
                "TOKEN=Bearer value",
                "--",
                "node",
                "srv.js"
            ]
        );
        let passed = stdio(&[("TOKEN", "${TOKEN}")]);
        assert_eq!(cli_args("srv", &passed).unwrap(), None);
    }

    #[test]
    fn file_edit_keeps_the_rest_and_round_trips() {
        let tmp = TempDir::new("agsync-test").unwrap();
        std::fs::write(tmp.path().join("config.toml"), CONFIG).unwrap();
        let codex = Codex::at(
            "codex-not-called".into(),
            tmp.path().into(),
            tmp.path().into(),
        );
        let spec = stdio(&[("MODE", "x"), ("TOKEN", "${TOKEN}")]);
        codex.set_entry("srv", native(&spec).unwrap()).unwrap();
        let text = std::fs::read_to_string(tmp.path().join("config.toml")).unwrap();
        assert!(text.starts_with(CONFIG), "{text}");
        assert!(text.contains("env_vars = [\"TOKEN\"]"), "{text}");
        assert_eq!(codex.mcps().unwrap()["srv"], spec.hash());

        let empty = TempDir::new("agsync-test").unwrap();
        let codex = Codex::at(
            "codex-not-called".into(),
            empty.path().into(),
            empty.path().into(),
        );
        codex.set_entry("srv", native(&spec).unwrap()).unwrap();
        let text = std::fs::read_to_string(empty.path().join("config.toml")).unwrap();
        assert!(text.starts_with("[mcp_servers.srv]"), "{text}");
    }
}
