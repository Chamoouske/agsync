//! What `apply` did, as plain data the CLI prints and the app draws.

use serde::Serialize;

use crate::plan::Action;
use crate::report::DiffView;

/// One step of an apply, sent as it happens and kept in [`Applied::events`].
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    BuildStarted {
        name: String,
        sha: String,
    },
    BuildDone {
        name: String,
    },
    /// The MCP of this build is skipped.
    BuildFailed {
        name: String,
        error: String,
    },
    /// Nothing of this agent is written.
    BackupFailed {
        agent: &'static str,
        error: String,
    },
    Written {
        agent: &'static str,
        kind: &'static str,
        name: String,
        action: Action,
    },
    ItemFailed {
        agent: &'static str,
        kind: &'static str,
        name: String,
        error: String,
    },
    StateSaveFailed {
        error: String,
    },
    Cleaned {
        count: usize,
    },
    CleanupFailed {
        error: String,
    },
}

/// Result of one apply: the plan it was asked about and what happened after.
#[derive(Debug, Serialize)]
pub struct Applied {
    /// Changes, conflicts and adoptions, builds with the import they come from.
    pub plan: DiffView,
    pub skipped: Vec<String>,
    /// Some build comes from an imported MCP: third-party code from another repo.
    pub imported_build: bool,
    /// False when there was nothing to apply; nothing was asked then.
    pub pending: bool,
    /// False when the answer was no; nothing was built or written then.
    pub confirmed: bool,
    pub events: Vec<Event>,
    /// Builds, backups and items that failed; the rest was applied.
    pub failures: usize,
    pub mcp_written: bool,
    /// What to do in each agent that had something written for the change to take effect.
    pub hints: Vec<String>,
}
