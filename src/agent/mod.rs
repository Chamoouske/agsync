//! Agent adapters: one per coding agent, all speaking the neutral MCP model.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::mcp::{McpSpec, RefSpot};
use crate::plugin::PluginSpec;
use crate::state::LinkMode;

pub mod antigravity;
pub mod backup;
pub mod claude;
pub mod cli;
pub mod codex;
pub mod gemini;
pub mod hermes;
pub mod json_config;
pub mod link;
pub mod opencode;

/// What agsync needs from an agent's user-level config.
pub trait Adapter {
    /// True if a `${VAR}` at `spot` may stay a reference, for the agent to expand from
    /// its own environment. By default every spot accepts one.
    fn keeps_ref(&self, _spot: RefSpot<'_>) -> bool {
        true
    }
    /// Current MCP entries: name to content hash.
    fn mcps(&self) -> anyhow::Result<BTreeMap<String, String>>;
    /// One MCP entry in the neutral form; `None` when absent or of a shape agsync does
    /// not write.
    fn mcp_spec(&self, name: &str) -> anyhow::Result<Option<McpSpec>>;
    /// Folder where agsync places this agent's skills.
    fn skills_dir(&self) -> PathBuf;
    /// Every folder the agent loads skills from. By default only [`Adapter::skills_dir`].
    fn skill_dirs_read(&self) -> Vec<PathBuf> {
        vec![self.skills_dir()]
    }
    /// Current skill folders: name to content hash.
    fn skills(&self) -> anyhow::Result<BTreeMap<String, String>> {
        link::installed(&self.skills_dir())
    }
    /// Copies the agent config file into `dir` before changes.
    fn backup(&self, dir: &Path) -> anyhow::Result<()>;
    /// Creates or replaces one MCP entry.
    fn write_mcp(&self, name: &str, spec: &McpSpec) -> anyhow::Result<()>;
    /// Removes one MCP entry.
    fn remove_mcp(&self, name: &str) -> anyhow::Result<()>;
    /// Places the cached skill folder under the agent's skills dir.
    fn write_skill(&self, name: &str, cached: &Path, backups: &Path) -> anyhow::Result<LinkMode> {
        link::install(cached, &self.skills_dir().join(name), backups)
    }
    /// Removes one skill folder or link.
    fn remove_skill(&self, name: &str, backups: &Path) -> anyhow::Result<()> {
        link::clear(&self.skills_dir().join(name), backups)
    }
    /// Checks once that the CLI has non-interactive plugin commands. The default is
    /// no support, with the reason shown by `status`.
    fn plugin_support(&self) -> Result<(), String> {
        Err("CLI sem comandos nao interativos de plugins".to_string())
    }
    /// Id the agent already uses for `spec`, if it is installed.
    fn plugin_installed(&self, _spec: &PluginSpec) -> anyhow::Result<Option<String>> {
        Ok(None)
    }
    /// Id of the only plugin installed under `name` from any source, if exactly one is.
    /// Lets a manual install of the same plugin from another marketplace be replaced.
    fn plugin_named(&self, _name: &str) -> anyhow::Result<Option<String>> {
        Ok(None)
    }
    /// True if the agent still has the plugin recorded under `id`.
    fn plugin_present(&self, _id: &str) -> anyhow::Result<bool> {
        Ok(false)
    }
    /// Installs or updates one plugin and returns the id to record. `managed` holds
    /// every plugin id agsync tracks for this agent; some CLIs need it to avoid
    /// touching plugins installed by hand.
    fn install_plugin(
        &self,
        _spec: &PluginSpec,
        _managed: &BTreeSet<String>,
    ) -> anyhow::Result<String> {
        anyhow::bail!("agente sem suporte a plugins")
    }
    /// Removes one plugin by the id returned by [`Adapter::install_plugin`].
    fn uninstall_plugin(&self, _id: &str) -> anyhow::Result<()> {
        anyhow::bail!("agente sem suporte a plugins")
    }
    /// Warnings `agsync doctor` shows for this agent; they are not problems.
    fn doctor_notes(&self) -> Vec<String> {
        Vec::new()
    }
    /// What to tell the user after agsync changed this agent, beyond restarting it.
    fn reload_hint(&self) -> Option<&'static str> {
        None
    }
}

/// Finds an agent on this machine, or says why it is skipped.
pub type Detect = fn() -> Result<Box<dyn Adapter>, String>;

/// Every agent agsync knows, in the order they are planned and shown. Adding an agent
/// is one module plus one line here.
pub const AGENTS: [(&str, Detect); 6] = [
    ("claude", || boxed(claude::Claude::detect())),
    ("codex", || boxed(codex::Codex::detect())),
    ("opencode", || boxed(opencode::OpenCode::detect())),
    ("hermes", || boxed(hermes::Hermes::detect())),
    ("gemini", || boxed(gemini::Gemini::detect())),
    ("antigravity", || boxed(antigravity::Antigravity::detect())),
];

/// Names of every known agent, in table order.
pub fn names() -> impl Iterator<Item = &'static str> {
    AGENTS.iter().map(|(name, _)| *name)
}

fn boxed<A: Adapter + 'static>(found: Result<A, String>) -> Result<Box<dyn Adapter>, String> {
    found.map(|adapter| Box::new(adapter) as Box<dyn Adapter>)
}
