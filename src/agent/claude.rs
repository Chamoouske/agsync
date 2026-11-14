//! Claude Code adapter.
//!
//! Reads user MCP servers from `.claude.json` (top-level `mcpServers`) and never
//! writes that file: changes go through `claude mcp add-json/remove --scope user`.
//! Skills are folders under `skills/`. Both live in `CLAUDE_CONFIG_DIR` when set,
//! else `~/.claude.json` and `~/.claude/skills`.

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::Context;
use serde::Deserialize;

use super::{Adapter, backup, cli};
use crate::mcp::{McpSpec, hash_value};
use crate::plugin::{PluginSpec, same_repo};

/// One line of `claude plugin marketplace list --json`.
#[derive(Debug, Clone, Deserialize)]
struct MarketplaceEntry {
    name: String,
    source: String,
    #[serde(default)]
    repo: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    path: Option<String>,
    #[serde(default, rename = "ref")]
    git_ref: Option<String>,
}

impl MarketplaceEntry {
    fn matches(&self, git: &str) -> bool {
        match self.source.as_str() {
            "github" => self
                .repo
                .as_deref()
                .is_some_and(|repo| same_repo(repo, git)),
            "git" => self.url.as_deref().is_some_and(|url| same_repo(url, git)),
            "directory" => self
                .path
                .as_deref()
                .is_some_and(|path| same_repo(path, git)),
            _ => false,
        }
    }

    /// Source for `claude plugin marketplace add` that brings this entry back.
    fn source_arg(&self) -> Option<String> {
        let base = match self.source.as_str() {
            "github" => self.repo.clone(),
            "git" => self.url.clone(),
            "directory" => self.path.clone(),
            _ => None,
        }?;
        match &self.git_ref {
            Some(git_ref) => Some(format!("{base}#{git_ref}")),
            None => Some(base),
        }
    }
}

/// One line of `claude plugin list --json`.
#[derive(Debug, Clone, Deserialize)]
struct InstalledPlugin {
    id: String,
    #[serde(default)]
    scope: Option<String>,
}

impl InstalledPlugin {
    /// Plugin name without the `@marketplace` part.
    fn name(&self) -> &str {
        self.id.split('@').next().unwrap_or(&self.id)
    }

    fn scope(&self) -> &str {
        self.scope.as_deref().unwrap_or("user")
    }
}

/// Last `marketplace list` and `plugin list`, cleared by every call that changes them.
#[derive(Debug, Default)]
struct Cache {
    marketplaces: Option<Vec<MarketplaceEntry>>,
    plugins: Option<Vec<InstalledPlugin>>,
}

/// Claude Code user-level config.
#[derive(Debug)]
pub struct Claude {
    exe: PathBuf,
    config_file: PathBuf,
    settings_file: PathBuf,
    skills_dir: PathBuf,
    cache: RefCell<Cache>,
}

impl Claude {
    /// Finds `claude` in PATH and resolves config locations.
    pub fn detect() -> Result<Claude, String> {
        let exe = crate::which::require("claude")?;
        if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
            return Ok(Claude::at(exe, &PathBuf::from(dir)));
        }
        let home = crate::paths::home().map_err(|e| e.to_string())?;
        Ok(Claude {
            exe,
            config_file: home.join(".claude.json"),
            settings_file: home.join(".claude").join("settings.json"),
            skills_dir: home.join(".claude").join("skills"),
            cache: RefCell::default(),
        })
    }

    /// Adapter for a `CLAUDE_CONFIG_DIR`-style folder.
    pub fn at(exe: PathBuf, config_dir: &Path) -> Claude {
        Claude {
            exe,
            config_file: config_dir.join(".claude.json"),
            settings_file: config_dir.join("settings.json"),
            skills_dir: config_dir.join("skills"),
            cache: RefCell::default(),
        }
    }

    fn servers(&self) -> anyhow::Result<serde_json::Map<String, serde_json::Value>> {
        let text = match std::fs::read_to_string(&self.config_file) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Default::default()),
            Err(e) => {
                return Err(e).with_context(|| format!("ler {}", self.config_file.display()));
            }
        };
        let root: serde_json::Value = serde_json::from_str(&text)
            .with_context(|| format!("{} invalido", self.config_file.display()))?;
        let servers = root.get("mcpServers").and_then(|v| v.as_object());
        Ok(servers.cloned().unwrap_or_default())
    }

    fn has_server(&self, name: &str) -> anyhow::Result<bool> {
        Ok(self.servers()?.contains_key(name))
    }

    /// Runs one `claude` call. Output is only echoed when it cannot hold secrets.
    fn run(&self, args: &[&str], show_output: bool) -> anyhow::Result<()> {
        self.cache.take();
        let mut cmd = Command::new(&self.exe);
        cmd.args(args);
        let what = format!("claude {}", args[..2].join(" "));
        cli::run(cmd, &what, show_output)
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(&self.exe);
        cmd.args(args);
        cmd
    }

    fn marketplaces(&self) -> anyhow::Result<Vec<MarketplaceEntry>> {
        if let Some(list) = &self.cache.borrow().marketplaces {
            return Ok(list.clone());
        }
        let list: Vec<MarketplaceEntry> = cli::json(
            self.command(&["plugin", "marketplace", "list", "--json"]),
            "claude plugin marketplace list",
        )?;
        self.cache.borrow_mut().marketplaces = Some(list.clone());
        Ok(list)
    }

    fn installed(&self) -> anyhow::Result<Vec<InstalledPlugin>> {
        if let Some(list) = &self.cache.borrow().plugins {
            return Ok(list.clone());
        }
        let list: Vec<InstalledPlugin> = cli::json(
            self.command(&["plugin", "list", "--json"]),
            "claude plugin list",
        )?;
        self.cache.borrow_mut().plugins = Some(list.clone());
        Ok(list)
    }

    /// Installed plugins that belong to the user, not to the running session.
    fn user_plugins(&self) -> anyhow::Result<Vec<InstalledPlugin>> {
        Ok(self
            .installed()?
            .into_iter()
            .filter(|p| p.scope.as_deref() != Some("session"))
            .collect())
    }

    fn marketplace_of(&self, spec: &PluginSpec) -> anyhow::Result<Option<MarketplaceEntry>> {
        Ok(self
            .marketplaces()?
            .into_iter()
            .find(|m| m.matches(&spec.git)))
    }

    /// Adds the marketplace at `source` and returns the name declared in its catalog.
    fn add_marketplace(&self, source: &str) -> anyhow::Result<String> {
        self.cache.take();
        let cmd = self.command(&[
            "plugin",
            "marketplace",
            "add",
            source,
            "--scope",
            "user",
            "--json",
        ]);
        let out: serde_json::Value = cli::json(cmd, "claude plugin marketplace add")?;
        let name = out
            .get("marketplace")
            .and_then(|v| v.as_str())
            .filter(|name| !name.is_empty())
            .context("claude plugin marketplace add nao retornou o nome do marketplace")?;
        Ok(name.to_string())
    }

    /// A marketplace cannot change ref in place: remove it and add it again.
    /// Refuses when the marketplace also holds plugins agsync does not manage. If the
    /// new ref cannot be added, the old ref and its plugins are put back.
    fn switch_marketplace(
        &self,
        found: &MarketplaceEntry,
        spec: &PluginSpec,
        managed: &BTreeSet<String>,
    ) -> anyhow::Result<String> {
        let suffix = format!("@{}", found.name);
        let installed: Vec<InstalledPlugin> = self
            .user_plugins()?
            .into_iter()
            .filter(|p| p.id.ends_with(&suffix))
            .collect();
        let untracked: Vec<&str> = installed
            .iter()
            .filter(|p| !managed.contains(&p.id))
            .map(|p| p.id.as_str())
            .collect();
        if !untracked.is_empty() {
            anyhow::bail!(
                "marketplace '{}' tambem tem plugins fora do agsync ({}); troque a ref a mao",
                found.name,
                untracked.join(", ")
            );
        }
        self.run(
            &[
                "plugin",
                "marketplace",
                "remove",
                &found.name,
                "--scope",
                "user",
                "--json",
            ],
            true,
        )?;
        let name = match self.add_marketplace(&spec.describe()) {
            Ok(name) => name,
            Err(e) => return Err(self.restore_marketplace(found, &installed, e)),
        };
        let others: Vec<InstalledPlugin> = installed
            .into_iter()
            .filter(|p| p.name() != spec.name)
            .collect();
        let failed = self.reinstall(&others, &name);
        if !failed.is_empty() {
            anyhow::bail!(
                "marketplace '{}' trocado, mas estes plugins nao voltaram: {}",
                name,
                failed.join("; ")
            );
        }
        Ok(name)
    }

    /// Adds `found` back at its old ref after the new one failed, with its plugins.
    /// Returns the error that says what came back and what did not.
    fn restore_marketplace(
        &self,
        found: &MarketplaceEntry,
        installed: &[InstalledPlugin],
        cause: anyhow::Error,
    ) -> anyhow::Error {
        let ids: Vec<&str> = installed.iter().map(|p| p.id.as_str()).collect();
        let lost = ids.join(", ");
        let Some(source) = found.source_arg() else {
            return anyhow::anyhow!(
                "ref nova falhou ({cause:#}); marketplace '{}' removido e sem origem para readicionar; plugins fora: {lost}",
                found.name
            );
        };
        let name = match self.add_marketplace(&source) {
            Ok(name) => name,
            Err(e) => {
                return anyhow::anyhow!(
                    "ref nova falhou ({cause:#}); marketplace '{}' removido e nao voltou ({e:#}); plugins fora: {lost}",
                    found.name
                );
            }
        };
        let failed = self.reinstall(installed, &name);
        if failed.is_empty() {
            return anyhow::anyhow!(
                "ref nova falhou ({cause:#}); marketplace '{name}' voltou para a ref antiga com seus plugins"
            );
        }
        anyhow::anyhow!(
            "ref nova falhou ({cause:#}); marketplace '{name}' voltou para a ref antiga, mas estes plugins nao voltaram: {}",
            failed.join("; ")
        )
    }

    /// Installs each plugin from `marketplace` in its own scope; returns the failures.
    fn reinstall(&self, plugins: &[InstalledPlugin], marketplace: &str) -> Vec<String> {
        let mut failed = Vec::new();
        for plugin in plugins {
            let id = format!("{}@{marketplace}", plugin.name());
            let args = [
                "plugin",
                "install",
                &id,
                "--scope",
                plugin.scope(),
                "--json",
                "-y",
            ];
            if let Err(e) = self.run(&args, true) {
                failed.push(format!("{id}: {e:#}"));
            }
        }
        failed
    }

    /// Marketplace name for `spec`, adding it or moving it to the requested ref.
    fn ensure_marketplace(
        &self,
        spec: &PluginSpec,
        managed: &BTreeSet<String>,
    ) -> anyhow::Result<String> {
        match self.marketplace_of(spec)? {
            Some(found) if spec.is_local() => Ok(found.name),
            Some(found) if found.git_ref.as_deref() == Some(spec.git_ref.as_str()) => {
                Ok(found.name)
            }
            Some(found) => self.switch_marketplace(&found, spec, managed),
            None => self.add_marketplace(&spec.describe()),
        }
    }
}

impl Adapter for Claude {
    fn mcps(&self) -> anyhow::Result<BTreeMap<String, String>> {
        let servers = self.servers()?;
        Ok(servers
            .iter()
            .map(|(name, value)| (name.clone(), hash_value(value)))
            .collect())
    }

    fn mcp_spec(&self, name: &str) -> anyhow::Result<Option<McpSpec>> {
        let Some(mut value) = self.servers()?.remove(name) else {
            return Ok(None);
        };
        if let Some(entry) = value.as_object_mut()
            && !entry.contains_key("type")
            && entry.contains_key("command")
        {
            entry.insert("type".into(), "stdio".into());
        }
        Ok(serde_json::from_value(value).ok())
    }

    fn skills_dir(&self) -> PathBuf {
        self.skills_dir.clone()
    }

    fn backup(&self, dir: &Path) -> anyhow::Result<()> {
        backup::copy(&self.config_file, dir, "claude.json")?;
        backup::copy(&self.settings_file, dir, "settings.json")
    }

    fn write_mcp(&self, name: &str, spec: &McpSpec) -> anyhow::Result<()> {
        if self.has_server(name)? {
            self.run(&["mcp", "remove", "--scope", "user", name], true)?;
        }
        let json = spec.to_json();
        self.run(&["mcp", "add-json", "--scope", "user", name, &json], false)?;
        let written = self.servers()?.get(name).map(hash_value);
        if written.as_deref() != Some(spec.hash().as_str()) {
            anyhow::bail!("claude nao gravou o mcp '{name}' como esperado");
        }
        Ok(())
    }

    fn remove_mcp(&self, name: &str) -> anyhow::Result<()> {
        if !self.has_server(name)? {
            return Ok(());
        }
        self.run(&["mcp", "remove", "--scope", "user", name], true)?;
        if self.has_server(name)? {
            anyhow::bail!("mcp '{name}' continua no arquivo apos remove");
        }
        Ok(())
    }

    fn plugin_support(&self) -> Result<(), String> {
        self.user_plugins()
            .map(|_| ())
            .map_err(|e| format!("`claude plugin list` falhou: {e:#}"))
    }

    fn plugin_installed(&self, spec: &PluginSpec) -> anyhow::Result<Option<String>> {
        let marketplaces: BTreeSet<String> = self
            .marketplaces()?
            .into_iter()
            .filter(|m| m.matches(&spec.git))
            .filter(|m| spec.is_local() || m.git_ref.as_deref() == Some(spec.git_ref.as_str()))
            .map(|m| m.name)
            .collect();
        let ids = self.user_plugins()?;
        Ok(ids
            .into_iter()
            .map(|p| p.id)
            .find(|id| match id.split_once('@') {
                Some((name, marketplace)) => {
                    name == spec.name && marketplaces.contains(marketplace)
                }
                None => false,
            }))
    }

    fn plugin_present(&self, id: &str) -> anyhow::Result<bool> {
        Ok(self.user_plugins()?.iter().any(|p| p.id == id))
    }

    fn install_plugin(
        &self,
        spec: &PluginSpec,
        managed: &BTreeSet<String>,
    ) -> anyhow::Result<String> {
        let marketplace = self.ensure_marketplace(spec, managed)?;
        let id = format!("{}@{marketplace}", spec.name);
        self.run(
            &["plugin", "install", &id, "--scope", "user", "--json", "-y"],
            true,
        )?;
        self.run(&["plugin", "update", &id, "--json", "-y"], true)?;
        Ok(id)
    }

    fn uninstall_plugin(&self, id: &str) -> anyhow::Result<()> {
        if !self.plugin_present(id)? {
            return Ok(());
        }
        self.run(
            &[
                "plugin",
                "uninstall",
                id,
                "--scope",
                "user",
                "--keep-data",
                "--json",
                "-y",
            ],
            true,
        )?;
        if self.plugin_present(id)? {
            anyhow::bail!("plugin '{id}' continua instalado apos uninstall");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::hash_dir;
    use crate::tmp::TempDir;

    #[test]
    fn reads_servers_and_skills_without_cli() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let dir = tmp.path();
        let claude = Claude::at(PathBuf::from("claude-not-called"), dir);
        assert!(claude.mcps().unwrap().is_empty());
        assert!(claude.skills().unwrap().is_empty());

        let spec = McpSpec::Stdio {
            command: "npx".into(),
            args: vec!["-y".into()],
            env: BTreeMap::new(),
        };
        let file = format!(
            r#"{{"userID":"x","mcpServers":{{"echo":{}}},"projects":{{}}}}"#,
            spec.to_json()
        );
        std::fs::write(dir.join(".claude.json"), file).unwrap();
        std::fs::create_dir_all(dir.join("skills").join("hello")).unwrap();
        std::fs::write(dir.join("skills").join("hello").join("SKILL.md"), "x").unwrap();

        assert_eq!(claude.mcps().unwrap()["echo"], spec.hash());
        let skills = claude.skills().unwrap();
        assert_eq!(
            skills["hello"],
            hash_dir(&dir.join("skills").join("hello")).unwrap()
        );

        let backups = dir.join("backups");
        claude.backup(&backups).unwrap();
        assert_eq!(std::fs::read_dir(&backups).unwrap().count(), 1);
    }

    #[test]
    fn marketplace_entry_matches_every_source_kind() {
        let github = MarketplaceEntry {
            name: "m".into(),
            source: "github".into(),
            repo: Some("owner/repo".into()),
            url: None,
            path: None,
            git_ref: None,
        };
        assert!(github.matches("owner/repo"));
        assert!(github.matches("https://github.com/owner/repo.git"));
        assert!(!github.matches("owner/other"));

        let git = MarketplaceEntry {
            name: "m".into(),
            source: "git".into(),
            repo: None,
            url: Some("https://gitlab.example.com/group/market.git".into()),
            path: None,
            git_ref: Some("v1".into()),
        };
        assert!(git.matches("https://gitlab.example.com/group/market"));
        assert!(git.matches("git@gitlab.example.com:group/market.git"));
        assert!(!git.matches("https://gitlab.example.com/group/other.git"));

        let tmp = TempDir::new("agsync-test").unwrap();
        let local = MarketplaceEntry {
            name: "m".into(),
            source: "directory".into(),
            repo: None,
            url: None,
            path: Some(tmp.path().to_string_lossy().to_string()),
            git_ref: None,
        };
        assert!(local.matches(&tmp.path().to_string_lossy()));

        let unknown = MarketplaceEntry {
            name: "m".into(),
            source: "npm".into(),
            repo: None,
            url: None,
            path: None,
            git_ref: None,
        };
        assert!(!unknown.matches("owner/repo"));
    }

    #[test]
    fn reads_one_entry_without_type_as_stdio() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let claude = Claude::at(PathBuf::from("claude-not-called"), tmp.path());
        assert_eq!(claude.mcp_spec("1mcp").unwrap(), None);
        let file = r#"{"mcpServers":{"1mcp":{"command":"1mcp","args":["proxy"]}}}"#;
        std::fs::write(tmp.path().join(".claude.json"), file).unwrap();
        let legacy = McpSpec::Stdio {
            command: "1mcp".into(),
            args: vec!["proxy".into()],
            env: BTreeMap::new(),
        };
        assert_eq!(claude.mcp_spec("1mcp").unwrap(), Some(legacy));
    }
}
