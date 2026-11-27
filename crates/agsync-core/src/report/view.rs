//! What `status` and `diff` show, as plain data the CLI prints and the app draws.

use serde::Serialize;

use super::{AgentPlan, Report};
use crate::compose::Origins;
use crate::plan::{Action, Change};
use crate::source::BuildJob;

/// `agsync status`: where the config comes from, which commits, and the plan.
#[derive(Debug, Serialize)]
pub struct StatusView {
    pub repo: String,
    pub git_ref: String,
    pub profiles: Vec<String>,
    /// Commit of the last apply, `None` before the first one.
    pub applied: Option<String>,
    pub remote: String,
    #[serde(flatten)]
    pub diff: DiffView,
    pub skipped: Vec<String>,
}

/// `agsync diff`: what apply would change, build and warn about.
#[derive(Debug, Serialize)]
pub struct DiffView {
    pub agents: Vec<AgentChanges>,
    pub builds: Vec<BuildView>,
    pub warnings: Vec<String>,
}

/// Changes of one detected agent, `Keep` left out.
#[derive(Debug, Serialize)]
pub struct AgentChanges {
    pub agent: &'static str,
    pub changes: Vec<ChangeView>,
}

#[derive(Debug, Serialize)]
pub struct ChangeView {
    pub kind: &'static str,
    pub name: String,
    pub action: Action,
    /// Source of the desired MCP or plugin; env and header values are never in it.
    pub detail: Option<String>,
    /// Import the item came from, `None` for the main repo's.
    pub origin: Option<String>,
}

/// One build apply would run: third-party code on this machine.
#[derive(Debug, Serialize)]
pub struct BuildView {
    pub name: String,
    pub repo: String,
    pub git_ref: String,
    pub sha: String,
    pub commands: Vec<String>,
    pub origin: Option<String>,
}

impl StatusView {
    pub fn of(report: &Report) -> StatusView {
        StatusView {
            repo: report.machine.repo.clone(),
            git_ref: report.machine.git_ref.clone(),
            profiles: report.machine.profiles.clone(),
            applied: report.state.commit.clone(),
            remote: report.remote.clone(),
            diff: DiffView::of(report),
            skipped: report.skipped.clone(),
        }
    }
}

impl DiffView {
    pub fn of(report: &Report) -> DiffView {
        let agents = report
            .agents
            .iter()
            .map(|plan| AgentChanges::of(plan, &report.origins))
            .collect();
        let builds = report
            .builds
            .iter()
            .map(|job| BuildView::of(job, &report.origins))
            .collect();
        DiffView {
            agents,
            builds,
            warnings: report.warnings.clone(),
        }
    }
}

impl AgentChanges {
    fn of(plan: &AgentPlan, origins: &Origins) -> AgentChanges {
        let changes = plan
            .visible()
            .map(|(kind, change)| ChangeView {
                kind,
                name: change.name.clone(),
                action: change.action,
                detail: detail(plan, kind, change),
                origin: origin(origins, kind, &change.name),
            })
            .collect();
        AgentChanges {
            agent: plan.agent,
            changes,
        }
    }
}

impl BuildView {
    fn of(job: &BuildJob, origins: &Origins) -> BuildView {
        BuildView {
            name: job.name.clone(),
            repo: job.repo.clone(),
            git_ref: job.git_ref.clone(),
            sha: job.sha.clone(),
            commands: job.commands.clone(),
            origin: origin(origins, "mcp", &job.name),
        }
    }
}

/// Source of a desired MCP or plugin, left out for a conflict: apply keeps the manual one.
fn detail(plan: &AgentPlan, kind: &str, change: &Change) -> Option<String> {
    if change.action == Action::Conflict {
        return None;
    }
    match kind {
        "mcp" => plan
            .desired
            .mcps
            .get(&change.name)
            .map(|want| want.shown.describe()),
        "plugin" => plan.desired.plugins.get(&change.name).map(|p| p.describe()),
        _ => None,
    }
}

/// `imports.<kind> <git>@<ref>` for an imported item, `None` for the main repo's.
pub fn origin(origins: &Origins, kind: &'static str, name: &str) -> Option<String> {
    origins.get(&(kind, name.to_string())).cloned()
}
