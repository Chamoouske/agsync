//! Codex plugins, through the CLI only.
//!
//! Marketplaces are added with `codex plugin marketplace add` and recorded by Codex in
//! `config.toml`, table `[marketplaces.<name>]` with `source_type`, `source` and `ref`;
//! agsync reads that table to find them. Plugins are installed with
//! `codex plugin add <plugin>@<marketplace>` and listed with `codex plugin list --json`.

use std::collections::BTreeSet;

use serde::Deserialize;

use super::Codex;
use crate::agent::cli;
use crate::plugin::{PluginSpec, same_repo};

/// One entry of `installed` in `codex plugin list --json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Installed {
    pub plugin_id: String,
    pub name: String,
    pub marketplace_name: String,
}

#[derive(Debug, Deserialize)]
struct PluginList {
    #[serde(default)]
    installed: Vec<Installed>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AddedMarketplace {
    marketplace_name: String,
}

/// One `[marketplaces.<name>]` table of `config.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Marketplace {
    name: String,
    source: String,
    git_ref: Option<String>,
}

impl Marketplace {
    fn same_source(&self, spec: &PluginSpec) -> bool {
        same_repo(&self.source, &spec.git)
    }

    fn at_ref(&self, spec: &PluginSpec) -> bool {
        spec.is_local() || self.git_ref.as_deref() == Some(spec.git_ref.as_str())
    }
}

/// Marketplaces recorded in a parsed `config.toml`.
fn marketplaces(config: &toml::Table) -> Vec<Marketplace> {
    let Some(table) = config.get("marketplaces").and_then(|v| v.as_table()) else {
        return Vec::new();
    };
    table
        .iter()
        .filter_map(|(name, entry)| {
            let source = entry.get("source")?.as_str()?;
            let git_ref = entry.get("ref").and_then(|v| v.as_str());
            Some(Marketplace {
                name: name.clone(),
                source: source.to_string(),
                git_ref: git_ref.map(str::to_string),
            })
        })
        .collect()
}

/// Arguments of `codex plugin marketplace add` for `source` at `git_ref`.
fn add_args(source: &str, git_ref: Option<&str>) -> Vec<String> {
    let mut args: Vec<String> = vec!["plugin".into(), "marketplace".into(), "add".into()];
    args.push(source.to_string());
    if let Some(git_ref) = git_ref {
        args.push("--ref".into());
        args.push(git_ref.to_string());
    }
    args.push("--json".into());
    args
}

impl Codex {
    pub(super) fn installed_plugins(&self) -> anyhow::Result<Vec<Installed>> {
        if let Some(list) = &*self.plugins.borrow() {
            return Ok(list.clone());
        }
        let list: PluginList = cli::json(
            self.command(&["plugin", "list", "--json"]),
            "codex plugin list",
        )?;
        *self.plugins.borrow_mut() = Some(list.installed.clone());
        Ok(list.installed)
    }

    pub(super) fn has_plugin(&self, id: &str) -> anyhow::Result<bool> {
        Ok(self.installed_plugins()?.iter().any(|p| p.plugin_id == id))
    }

    fn marketplaces(&self) -> anyhow::Result<Vec<Marketplace>> {
        Ok(marketplaces(&self.table()?))
    }

    pub(super) fn plugin_from_marketplace(
        &self,
        spec: &PluginSpec,
    ) -> anyhow::Result<Option<String>> {
        let names: BTreeSet<String> = self
            .marketplaces()?
            .into_iter()
            .filter(|m| m.same_source(spec) && m.at_ref(spec))
            .map(|m| m.name)
            .collect();
        Ok(self
            .installed_plugins()?
            .into_iter()
            .find(|p| p.name == spec.name && names.contains(&p.marketplace_name))
            .map(|p| p.plugin_id))
    }

    pub(super) fn only_plugin_named(&self, name: &str) -> anyhow::Result<Option<String>> {
        let ids: Vec<String> = self
            .installed_plugins()?
            .into_iter()
            .filter(|p| p.name == name)
            .map(|p| p.plugin_id)
            .collect();
        Ok(match ids.as_slice() {
            [id] => Some(id.clone()),
            _ => None,
        })
    }

    /// Adds the marketplace and returns the name declared in its catalog.
    fn add_marketplace(&self, source: &str, git_ref: Option<&str>) -> anyhow::Result<String> {
        self.plugins.take();
        let args = add_args(source, git_ref);
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        let out: AddedMarketplace = cli::json(self.command(&args), "codex plugin marketplace add")?;
        if out.marketplace_name.is_empty() {
            anyhow::bail!("codex plugin marketplace add nao retornou o nome do marketplace");
        }
        Ok(out.marketplace_name)
    }

    fn remove_marketplace(&self, name: &str) -> anyhow::Result<()> {
        let args = ["plugin", "marketplace", "remove", name, "--json"];
        self.run(&args.map(String::from), true)
    }

    /// Marketplace name for `spec`, adding it or moving it to the requested ref.
    fn ensure_marketplace(
        &self,
        spec: &PluginSpec,
        managed: &BTreeSet<String>,
    ) -> anyhow::Result<String> {
        let found = self
            .marketplaces()?
            .into_iter()
            .find(|m| m.same_source(spec));
        match found {
            Some(found) if found.at_ref(spec) => Ok(found.name),
            Some(found) => self.switch_marketplace(&found, spec, managed),
            None if spec.is_local() => self.add_marketplace(&spec.git, None),
            None => self.add_marketplace(&spec.clone_url(), Some(&spec.git_ref)),
        }
    }

    /// Codex keeps one ref per marketplace: remove it and add it at the new ref.
    /// Installed plugins stay installed meanwhile; each managed one is reinstalled by
    /// its own change. Refuses when the marketplace also holds plugins agsync does not
    /// manage. If the new ref cannot be added, the old one is added back.
    fn switch_marketplace(
        &self,
        found: &Marketplace,
        spec: &PluginSpec,
        managed: &BTreeSet<String>,
    ) -> anyhow::Result<String> {
        let untracked: Vec<String> = self
            .installed_plugins()?
            .into_iter()
            .filter(|p| p.marketplace_name == found.name && !managed.contains(&p.plugin_id))
            .map(|p| p.plugin_id)
            .collect();
        if !untracked.is_empty() {
            anyhow::bail!(
                "marketplace '{}' tambem tem plugins fora do agsync ({}); troque a ref a mao",
                found.name,
                untracked.join(", ")
            );
        }
        self.remove_marketplace(&found.name)?;
        let cause = match self.add_marketplace(&spec.clone_url(), Some(&spec.git_ref)) {
            Ok(name) => return Ok(name),
            Err(e) => e,
        };
        match self.add_marketplace(&found.source, found.git_ref.as_deref()) {
            Ok(name) => anyhow::bail!(
                "ref nova falhou ({cause:#}); marketplace '{name}' voltou para a ref antiga"
            ),
            Err(e) => anyhow::bail!(
                "ref nova falhou ({cause:#}); marketplace '{}' removido e nao voltou ({e:#}); readicione com `codex plugin marketplace add`",
                found.name
            ),
        }
    }

    pub(super) fn add_plugin(
        &self,
        spec: &PluginSpec,
        managed: &BTreeSet<String>,
    ) -> anyhow::Result<String> {
        let marketplace = self.ensure_marketplace(spec, managed)?;
        let id = format!("{}@{marketplace}", spec.name);
        let args = ["plugin", "add", &id, "--json"];
        self.run(&args.map(String::from), true)?;
        if !self.has_plugin(&id)? {
            anyhow::bail!("codex nao instalou o plugin '{id}'");
        }
        Ok(id)
    }

    pub(super) fn remove_plugin(&self, id: &str) -> anyhow::Result<()> {
        if !self.has_plugin(id)? {
            return Ok(());
        }
        let args = ["plugin", "remove", id, "--json"];
        self.run(&args.map(String::from), true)?;
        if self.has_plugin(id)? {
            anyhow::bail!("plugin '{id}' continua instalado apos remove");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_marketplaces_with_and_without_ref() {
        let config: toml::Table = toml::from_str(
            r#"
model = "x"

[marketplaces.team]
source_type = "git"
source = "https://github.com/owner/market.git"
ref = "v1"

[marketplaces.local]
source_type = "local"
source = "/srv/market"

[marketplaces.broken]
source_type = "git"
"#,
        )
        .unwrap();
        let found = marketplaces(&config);
        assert_eq!(found.len(), 2);
        let team = found.iter().find(|m| m.name == "team").unwrap();
        assert_eq!(team.git_ref.as_deref(), Some("v1"));
        let local = found.iter().find(|m| m.name == "local").unwrap();
        assert_eq!(local.git_ref, None);
        assert!(marketplaces(&toml::Table::new()).is_empty());
    }

    #[test]
    fn matches_source_and_ref_of_the_spec() {
        let market = Marketplace {
            name: "team".into(),
            source: "https://github.com/owner/market.git".into(),
            git_ref: Some("v1".into()),
        };
        let v1 = PluginSpec::new("p1", "owner/market", "v1");
        let v2 = PluginSpec::new("p1", "owner/market", "v2");
        let other = PluginSpec::new("p1", "owner/other", "v1");
        assert!(market.same_source(&v1) && market.at_ref(&v1));
        assert!(market.same_source(&v2) && !market.at_ref(&v2));
        assert!(!market.same_source(&other));

        let local = Marketplace {
            name: "local".into(),
            source: "./market".into(),
            git_ref: None,
        };
        assert!(local.at_ref(&PluginSpec::new("p1", "./market", "v9")));
    }

    #[test]
    fn add_args_pass_ref_only_when_given() {
        assert_eq!(
            add_args("https://x/m.git", Some("v1")),
            [
                "plugin",
                "marketplace",
                "add",
                "https://x/m.git",
                "--ref",
                "v1",
                "--json"
            ]
        );
        assert_eq!(
            add_args("./m", None),
            ["plugin", "marketplace", "add", "./m", "--json"]
        );
    }

    #[test]
    fn parses_plugin_list_json() {
        let text = r#"{"installed":[{"pluginId":"p1@team","name":"p1","marketplaceName":"team","version":"1.0.0","installed":true,"enabled":true,"source":{"source":"local","path":"/x"},"installPolicy":"AVAILABLE","authPolicy":"ON_USE"}],"available":[]}"#;
        let list: PluginList = serde_json::from_str(text).unwrap();
        assert_eq!(list.installed.len(), 1);
        assert_eq!(list.installed[0].plugin_id, "p1@team");
        assert_eq!(list.installed[0].marketplace_name, "team");
    }
}
