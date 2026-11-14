//! Agent adapters: one per coding agent, all speaking the neutral MCP model.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::mcp::{McpSpec, RefSpot};
use crate::state::LinkMode;

pub mod backup;
pub mod claude;
pub mod cli;
pub mod codex;
pub mod hermes;
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
    /// Warnings `agsync doctor` shows for this agent; they are not problems.
    fn doctor_notes(&self) -> Vec<String> {
        Vec::new()
    }
    /// What to tell the user after agsync changed this agent, beyond restarting it.
    fn reload_hint(&self) -> Option<&'static str> {
        None
    }
}

/// Adapter for `agent`, or the reason it is skipped on this machine.
pub fn detect(agent: &str) -> Result<Box<dyn Adapter>, String> {
    match agent {
        "claude" => claude::Claude::detect().map(|c| Box::new(c) as Box<dyn Adapter>),
        "codex" => codex::Codex::detect().map(|c| Box::new(c) as Box<dyn Adapter>),
        "hermes" => hermes::Hermes::detect().map(|c| Box::new(c) as Box<dyn Adapter>),
        "opencode" => opencode::OpenCode::detect().map(|c| Box::new(c) as Box<dyn Adapter>),
        _ => Err(format!("agente desconhecido: {agent}")),
    }
}
