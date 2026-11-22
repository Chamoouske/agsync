//! Antigravity CLI (`agy`) adapter.
//!
//! Detected by `agy` in PATH. Config lives in `~/.gemini/config`.
//!
//! MCP servers are the top-level `mcpServers` of `mcp_config.json`, edited in place
//! with comments kept, because `agy` has no non-interactive MCP command. Remote servers
//! use `serverUrl` (streamable HTTP). Antigravity does not expand `${VAR}`, so secret
//! values are written into the file. Skills are folders under `skills/`; `agy` also
//! loads `~/.gemini/antigravity-cli/skills` and `~/.gemini/skills`.
//!
//! Plugins: `agy plugin install <folder>` and `agy plugin uninstall <name>`. `agy` takes
//! no ref, so a remote marketplace is checked out at the ref first and the plugin is
//! installed from that checkout. The `name` in `plugin.json` is the plugin name. agsync
//! marks what it installed with `plugins/<name>/.agsync-source.json`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

use super::json_config::{self, JsonConfig};
use super::{Adapter, backup, cli};
use crate::mcp::{McpSpec, RefSpot};
use crate::plugin::PluginSpec;

const URL_KEY: &str = "serverUrl";
const MARK: &str = ".agsync-source.json";

/// Antigravity CLI user config.
#[derive(Debug)]
pub struct Antigravity {
    agy: PathBuf,
    gemini_dir: PathBuf,
    config: JsonConfig,
}

impl Antigravity {
    /// Finds `agy` in PATH.
    pub fn detect() -> Result<Antigravity, String> {
        let agy = crate::which::require("agy")?;
        let home = crate::paths::home().map_err(|e| e.to_string())?;
        Ok(Antigravity::at(agy, home.join(".gemini")))
    }

    /// Adapter for an explicit `~/.gemini` folder.
    pub fn at(agy: PathBuf, gemini_dir: PathBuf) -> Antigravity {
        let config = JsonConfig::new(
            gemini_dir.join("config").join("mcp_config.json"),
            "mcpServers",
        );
        Antigravity {
            agy,
            gemini_dir,
            config,
        }
    }

    fn plugin_dir(&self, name: &str) -> PathBuf {
        self.gemini_dir.join("config").join("plugins").join(name)
    }

    fn written_hash(&self, name: &str) -> anyhow::Result<Option<String>> {
        Ok(self.config.servers()?.get(name).map(entry_hash))
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(&self.agy);
        cmd.args(args);
        cmd
    }

    /// Runs `agy plugin install` from the marketplace folder, or from a checkout of
    /// the remote marketplace at the ref.
    fn install_from_source(&self, spec: &PluginSpec) -> anyhow::Result<()> {
        if spec.is_local() {
            let cmd = self.command(&["plugin", "install", &spec.git]);
            return cli::run(cmd, "agy plugin install", true);
        }
        let url = spec.clone_url();
        let sha = crate::git::resolve(&url, &spec.git_ref)?;
        let snapshot = crate::git::Snapshot::fetch(&url, &spec.git_ref, &sha)?;
        let folder = snapshot.path().to_string_lossy().to_string();
        let cmd = self.command(&["plugin", "install", &folder]);
        let installed = cli::run(cmd, "agy plugin install", true);
        snapshot.close()?;
        installed
    }
}

impl Adapter for Antigravity {
    fn keeps_ref(&self, _spot: RefSpot<'_>) -> bool {
        false
    }

    fn mcps(&self) -> anyhow::Result<BTreeMap<String, String>> {
        let servers = self.config.servers()?;
        Ok(servers
            .iter()
            .map(|(name, value)| (name.clone(), entry_hash(value)))
            .collect())
    }

    fn mcp_spec(&self, name: &str) -> anyhow::Result<Option<McpSpec>> {
        Ok(self.config.servers()?.get(name).and_then(to_spec))
    }

    fn skills_dir(&self) -> PathBuf {
        self.gemini_dir.join("config").join("skills")
    }

    fn skill_dirs_read(&self) -> Vec<PathBuf> {
        vec![
            self.skills_dir(),
            self.gemini_dir.join("antigravity-cli").join("skills"),
            self.gemini_dir.join("skills"),
        ]
    }

    fn backup(&self, dir: &Path) -> anyhow::Result<()> {
        backup::copy(&self.config.file, dir, "antigravity-mcp_config.json")
    }

    fn write_mcp(&self, name: &str, spec: &McpSpec) -> anyhow::Result<()> {
        self.config.set(name, &json_config::entry(spec, URL_KEY))?;
        if self.written_hash(name)?.as_deref() != Some(spec.hash().as_str()) {
            anyhow::bail!("antigravity: mcp '{name}' nao ficou como esperado");
        }
        Ok(())
    }

    fn remove_mcp(&self, name: &str) -> anyhow::Result<()> {
        self.config.remove(name)?;
        if self.written_hash(name)?.is_some() {
            anyhow::bail!("mcp '{name}' continua em {}", self.config.file.display());
        }
        Ok(())
    }

    fn plugin_support(&self) -> Result<(), String> {
        cli::output(self.command(&["plugin", "--help"]), "agy plugin --help")
            .map(|_| ())
            .map_err(|e| format!("`agy plugin` indisponivel: {e:#}"))
    }

    fn plugin_installed(&self, spec: &PluginSpec) -> anyhow::Result<Option<String>> {
        if !self.plugin_present(&spec.name)? {
            return Ok(None);
        }
        let mark = json_config::read(&self.plugin_dir(&spec.name).join(MARK))?;
        let hash = mark
            .as_ref()
            .and_then(|m| m.get("hash"))
            .and_then(Value::as_str);
        Ok((hash == Some(spec.hash().as_str())).then(|| spec.name.clone()))
    }

    fn plugin_named(&self, name: &str) -> anyhow::Result<Option<String>> {
        Ok(self.plugin_present(name)?.then(|| name.to_string()))
    }

    fn plugin_present(&self, id: &str) -> anyhow::Result<bool> {
        Ok(self.plugin_dir(id).is_dir())
    }

    fn install_plugin(
        &self,
        spec: &PluginSpec,
        _managed: &BTreeSet<String>,
    ) -> anyhow::Result<String> {
        self.uninstall_plugin(&spec.name)?;
        self.install_from_source(spec)?;
        let dir = self.plugin_dir(&spec.name);
        if !dir.is_dir() {
            anyhow::bail!(
                "agy instalou o plugin, mas nao com o nome '{}'; confira o name do plugin.json",
                spec.name
            );
        }
        let mark = json!({"git": spec.git, "ref": spec.git_ref, "hash": spec.hash()});
        crate::atomic::write_atomic(&dir.join(MARK), mark.to_string().as_bytes())?;
        Ok(spec.name.clone())
    }

    fn uninstall_plugin(&self, id: &str) -> anyhow::Result<()> {
        if !self.plugin_present(id)? {
            return Ok(());
        }
        let cmd = self.command(&["plugin", "uninstall", id]);
        cli::run(cmd, "agy plugin uninstall", true)?;
        if self.plugin_present(id)? {
            anyhow::bail!("plugin '{id}' continua instalado no antigravity");
        }
        Ok(())
    }

    fn reload_hint(&self) -> Option<&'static str> {
        Some("antigravity: no agy, rode `/mcp` e `/skills reload` para carregar as mudancas")
    }
}

fn entry_hash(value: &Value) -> String {
    json_config::entry_hash(value, to_spec(value))
}

/// The neutral form of an entry; `url` is read as an alias of `serverUrl`.
fn to_spec(value: &Value) -> Option<McpSpec> {
    if value.get(URL_KEY).is_none() && value.get("url").is_some() {
        return json_config::spec_of(value, "url");
    }
    json_config::spec_of(value, URL_KEY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmp::TempDir;

    #[test]
    fn reads_and_writes_mcp_config_without_cli() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let ag = Antigravity::at("agy-not-called".into(), tmp.path().into());
        assert!(ag.mcps().unwrap().is_empty());
        let http = McpSpec::Http {
            url: "https://x/mcp".into(),
            headers: BTreeMap::from([("A".to_string(), "Bearer v".to_string())]),
        };
        ag.write_mcp("remote", &http).unwrap();
        let text = std::fs::read_to_string(&ag.config.file).unwrap();
        assert!(text.contains(r#""serverUrl": "https://x/mcp""#), "{text}");
        assert_eq!(ag.mcp_spec("remote").unwrap(), Some(http.clone()));

        let alias = json!({"url": "https://x/mcp", "headers": {"A": "Bearer v"}});
        assert_eq!(to_spec(&alias), Some(http));
        ag.remove_mcp("remote").unwrap();
        assert!(ag.mcps().unwrap().is_empty());
        assert!(!ag.keeps_ref(RefSpot::Command("${X}")));
    }

    #[test]
    fn installed_plugin_needs_the_mark_of_the_same_spec() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let ag = Antigravity::at("agy-not-called".into(), tmp.path().into());
        let spec = PluginSpec::new("p", "owner/p", "v1");
        assert_eq!(ag.plugin_installed(&spec).unwrap(), None);
        let dir = ag.plugin_dir("p");
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(ag.plugin_installed(&spec).unwrap(), None);
        assert_eq!(ag.plugin_named("p").unwrap().as_deref(), Some("p"));
        let mark = json!({"hash": spec.hash()}).to_string();
        std::fs::write(dir.join(MARK), mark).unwrap();
        assert_eq!(ag.plugin_installed(&spec).unwrap().as_deref(), Some("p"));
        let newer = PluginSpec::new("p", "owner/p", "v2");
        assert_eq!(ag.plugin_installed(&newer).unwrap(), None);
    }
}
