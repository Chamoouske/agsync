//! Gemini CLI adapter.
//!
//! Root: `GEMINI_CLI_HOME`, else the user home; every `gemini` call gets it explicitly.
//! Config lives in `<root>/.gemini`.
//!
//! MCP servers are the top-level `mcpServers` of `settings.json`, edited in place with
//! comments kept: `gemini mcp add` takes env values and headers as arguments, which
//! would expose secrets in the process list. Remote servers use `httpUrl` (streamable
//! HTTP); an entry with `url` is SSE, which agsync does not write. Gemini expands
//! `${VAR}` itself. Skills are folders under `<root>/.gemini/skills`; Gemini also loads
//! `<root>/.agents/skills`.
//!
//! Plugins are Gemini extensions: `gemini extensions install <git> --ref <ref> --consent`.
//! The extension repo is the `marketplace.git` and its `gemini-extension.json` name is
//! the plugin name. Gemini records the source and ref of each install in
//! `extensions/<name>/.gemini-extension-install.json`.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use super::json_config::{self, JsonConfig};
use super::{Adapter, backup, cli};
use crate::mcp::McpSpec;
use crate::plugin::{PluginSpec, same_repo};

const URL_KEY: &str = "httpUrl";
const INSTALL_METADATA: &str = ".gemini-extension-install.json";

/// Gemini CLI user config.
#[derive(Debug)]
pub struct Gemini {
    exe: PathBuf,
    root: PathBuf,
    config: JsonConfig,
}

impl Gemini {
    /// Finds `gemini` in PATH and resolves its root.
    pub fn detect() -> Result<Gemini, String> {
        let exe = crate::which::require("gemini")?;
        let root = match std::env::var_os("GEMINI_CLI_HOME") {
            Some(dir) => PathBuf::from(dir),
            None => crate::paths::home().map_err(|e| e.to_string())?,
        };
        Ok(Gemini::at(exe, root))
    }

    /// Adapter for an explicit root, the folder that holds `.gemini`.
    pub fn at(exe: PathBuf, root: PathBuf) -> Gemini {
        let config = JsonConfig::new(root.join(".gemini").join("settings.json"), "mcpServers");
        Gemini { exe, root, config }
    }

    fn dir(&self) -> PathBuf {
        self.root.join(".gemini")
    }

    fn extension_dir(&self, name: &str) -> PathBuf {
        self.dir().join("extensions").join(name)
    }

    fn written_hash(&self, name: &str) -> anyhow::Result<Option<String>> {
        Ok(self.config.servers()?.get(name).map(entry_hash))
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(&self.exe);
        cmd.env("GEMINI_CLI_HOME", &self.root).args(args);
        cmd
    }

    /// Source and ref Gemini recorded when it installed the extension `name`.
    fn install_metadata(&self, name: &str) -> anyhow::Result<Option<Value>> {
        json_config::read(&self.extension_dir(name).join(INSTALL_METADATA))
    }
}

impl Adapter for Gemini {
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
        self.dir().join("skills")
    }

    fn skill_dirs_read(&self) -> Vec<PathBuf> {
        vec![self.skills_dir(), self.root.join(".agents").join("skills")]
    }

    fn backup(&self, dir: &Path) -> anyhow::Result<()> {
        backup::copy(&self.config.file, dir, "gemini-settings.json")
    }

    fn write_mcp(&self, name: &str, spec: &McpSpec) -> anyhow::Result<()> {
        self.config.set(name, &json_config::entry(spec, URL_KEY))?;
        if self.written_hash(name)?.as_deref() != Some(spec.hash().as_str()) {
            anyhow::bail!("gemini: mcp '{name}' nao ficou como esperado");
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
        cli::output(
            self.command(&["extensions", "--help"]),
            "gemini extensions --help",
        )
        .map(|_| ())
        .map_err(|e| format!("`gemini extensions` indisponivel: {e:#}"))
    }

    fn plugin_installed(&self, spec: &PluginSpec) -> anyhow::Result<Option<String>> {
        let Some(meta) = self.install_metadata(&spec.name)? else {
            return Ok(None);
        };
        let source = meta.get("source").and_then(Value::as_str);
        let same_source = source.is_some_and(|s| same_repo(s, &spec.clone_url()));
        let git_ref = meta.get("ref").and_then(Value::as_str);
        let same_ref = spec.is_local() || git_ref == Some(spec.git_ref.as_str());
        Ok((same_source && same_ref).then(|| spec.name.clone()))
    }

    fn plugin_named(&self, name: &str) -> anyhow::Result<Option<String>> {
        Ok(self.plugin_present(name)?.then(|| name.to_string()))
    }

    fn plugin_present(&self, id: &str) -> anyhow::Result<bool> {
        Ok(self
            .extension_dir(id)
            .join("gemini-extension.json")
            .is_file())
    }

    fn install_plugin(
        &self,
        spec: &PluginSpec,
        _managed: &BTreeSet<String>,
    ) -> anyhow::Result<String> {
        self.uninstall_plugin(&spec.name)?;
        let source = spec.clone_url();
        let mut args = vec!["extensions", "install", source.as_str(), "--consent"];
        if !spec.is_local() {
            args.extend(["--ref", spec.git_ref.as_str()]);
        }
        cli::run(self.command(&args), "gemini extensions install", true)?;
        if !self.plugin_present(&spec.name)? {
            anyhow::bail!(
                "gemini instalou a extensao, mas nao com o nome '{}'; confira o name do gemini-extension.json",
                spec.name
            );
        }
        Ok(spec.name.clone())
    }

    fn uninstall_plugin(&self, id: &str) -> anyhow::Result<()> {
        if !self.plugin_present(id)? {
            return Ok(());
        }
        cli::run(
            self.command(&["extensions", "uninstall", id]),
            "gemini extensions uninstall",
            true,
        )?;
        if self.plugin_present(id)? {
            anyhow::bail!("extensao '{id}' continua instalada no gemini");
        }
        Ok(())
    }
}

fn entry_hash(value: &Value) -> String {
    json_config::entry_hash(value, to_spec(value))
}

/// The neutral form of a Gemini entry; SSE (`url`) is not one agsync writes.
fn to_spec(value: &Value) -> Option<McpSpec> {
    if value.get("url").is_some() {
        return None;
    }
    json_config::spec_of(value, URL_KEY)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmp::TempDir;

    const SETTINGS: &str = r#"{
  // keep me
  "theme": "Default",
  "mcpServers": {
    "manual": { "command": "tool", "args": ["a"], "trust": true },
    "sse": { "url": "https://x/sse" },
    "remote": { "httpUrl": "https://x/mcp", "headers": { "A": "${T}" } }
  }
}
"#;

    #[test]
    fn reads_and_writes_settings_without_cli() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let gemini = Gemini::at("gemini-not-called".into(), tmp.path().into());
        assert!(gemini.mcps().unwrap().is_empty());
        std::fs::create_dir_all(tmp.path().join(".gemini")).unwrap();
        std::fs::write(tmp.path().join(".gemini").join("settings.json"), SETTINGS).unwrap();

        let manual = McpSpec::Stdio {
            command: "tool".into(),
            args: vec!["a".into()],
            env: BTreeMap::new(),
        };
        let mcps = gemini.mcps().unwrap();
        assert_eq!(mcps["manual"], manual.hash());
        assert_eq!(gemini.mcp_spec("sse").unwrap(), None);
        let remote = McpSpec::Http {
            url: "https://x/mcp".into(),
            headers: BTreeMap::from([("A".to_string(), "${T}".to_string())]),
        };
        assert_eq!(gemini.mcp_spec("remote").unwrap(), Some(remote.clone()));

        gemini.write_mcp("added", &remote).unwrap();
        gemini.remove_mcp("manual").unwrap();
        let text = std::fs::read_to_string(&gemini.config.file).unwrap();
        assert!(text.contains("// keep me"), "{text}");
        assert!(text.contains(r#""httpUrl": "https://x/mcp""#), "{text}");
        let names: Vec<String> = gemini.mcps().unwrap().into_keys().collect();
        assert_eq!(names, ["added", "remote", "sse"]);
        assert_eq!(
            gemini.skill_dirs_read(),
            [
                tmp.path().join(".gemini").join("skills"),
                tmp.path().join(".agents").join("skills")
            ]
        );
    }

    #[test]
    fn installed_extension_matches_source_and_ref() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let gemini = Gemini::at("gemini-not-called".into(), tmp.path().into());
        let spec = PluginSpec::new("ext", "owner/ext", "v1");
        assert_eq!(gemini.plugin_installed(&spec).unwrap(), None);
        assert!(!gemini.plugin_present("ext").unwrap());

        let dir = gemini.extension_dir("ext");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("gemini-extension.json"), r#"{"name":"ext"}"#).unwrap();
        let meta = r#"{"source":"https://github.com/owner/ext.git","type":"git","ref":"v1"}"#;
        std::fs::write(dir.join(INSTALL_METADATA), meta).unwrap();

        assert!(gemini.plugin_present("ext").unwrap());
        assert_eq!(gemini.plugin_named("ext").unwrap().as_deref(), Some("ext"));
        assert_eq!(
            gemini.plugin_installed(&spec).unwrap().as_deref(),
            Some("ext")
        );
        let other_ref = PluginSpec::new("ext", "owner/ext", "v2");
        assert_eq!(gemini.plugin_installed(&other_ref).unwrap(), None);
        let other_repo = PluginSpec::new("ext", "owner/fork", "v1");
        assert_eq!(gemini.plugin_installed(&other_repo).unwrap(), None);
    }
}
