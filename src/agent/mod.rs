//! Agent adapters: one per coding agent, all speaking the neutral MCP model.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::mcp::McpSpec;
use crate::state::LinkMode;

pub mod claude;
pub mod link;

/// Reason shown for agents without an adapter yet (milestone 4).
pub const NOT_IMPLEMENTED: &str = "adaptador ainda nao implementado";

/// What agsync needs from an agent's user-level config.
pub trait Adapter {
    /// Current MCP entries: name to content hash.
    fn mcps(&self) -> anyhow::Result<BTreeMap<String, String>>;
    /// Current skill folders: name to content hash.
    fn skills(&self) -> anyhow::Result<BTreeMap<String, String>>;
    /// Copies the agent config file into `dir` before changes.
    fn backup(&self, dir: &Path) -> anyhow::Result<()>;
    /// Creates or replaces one MCP entry.
    fn write_mcp(&self, name: &str, spec: &McpSpec) -> anyhow::Result<()>;
    /// Removes one MCP entry.
    fn remove_mcp(&self, name: &str) -> anyhow::Result<()>;
    /// Places the cached skill folder under the agent's skills dir.
    fn write_skill(&self, name: &str, cached: &Path, backups: &Path) -> anyhow::Result<LinkMode>;
    /// Removes one skill folder or link.
    fn remove_skill(&self, name: &str, backups: &Path) -> anyhow::Result<()>;
}

/// Adapter for `agent`, or the reason it is skipped on this machine.
pub fn detect(agent: &str) -> Result<Box<dyn Adapter>, String> {
    match agent {
        "claude" => claude::Claude::detect().map(|c| Box::new(c) as Box<dyn Adapter>),
        _ => Err(NOT_IMPLEMENTED.to_string()),
    }
}

/// First `name` executable found in `PATH` (with `.exe`, `.cmd`, `.bat` on Windows).
pub fn find_in_path(name: &str) -> Option<PathBuf> {
    let exts: &[&str] = if cfg!(windows) {
        &[".exe", ".cmd", ".bat"]
    } else {
        &[""]
    };
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .flat_map(|dir| exts.iter().map(move |ext| dir.join(format!("{name}{ext}"))))
        .find(|candidate| candidate.is_file())
}
