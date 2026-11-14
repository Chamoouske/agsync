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

/// Finds an agent on this machine, or says why it is skipped.
pub type Detect = fn() -> Result<Box<dyn Adapter>, String>;

/// Every agent agsync knows, in the order they are planned and shown. Adding an agent
/// is one module plus one line here.
pub const AGENTS: [(&str, Detect); 4] = [
    ("claude", || boxed(claude::Claude::detect())),
    ("codex", || boxed(codex::Codex::detect())),
    ("opencode", || boxed(opencode::OpenCode::detect())),
    ("hermes", || boxed(hermes::Hermes::detect())),
];

/// Names of every known agent, in table order.
pub fn names() -> impl Iterator<Item = &'static str> {
    AGENTS.iter().map(|(name, _)| *name)
}

fn boxed<A: Adapter + 'static>(found: Result<A, String>) -> Result<Box<dyn Adapter>, String> {
    found.map(|adapter| Box::new(adapter) as Box<dyn Adapter>)
}
