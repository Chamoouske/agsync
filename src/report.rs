//! Snapshot the config repo and plan every agent: shared by status, diff and apply.

use crate::agent::{self, Adapter};
use crate::desired::{Desired, skipped_git_items};
use crate::git::{self, Snapshot};
use crate::machine::Machine;
use crate::manifest::{AGENTS, Manifest};
use crate::paths::Dirs;
use crate::plan::{Action, Change, plan};
use crate::state::{State, keys};

/// Plan for one agent that has an adapter on this machine.
pub struct AgentPlan {
    pub agent: &'static str,
    pub adapter: Box<dyn Adapter>,
    pub desired: Desired,
    pub mcps: Vec<Change>,
    pub skills: Vec<Change>,
}

impl AgentPlan {
    /// Changes worth showing (everything but `Keep`).
    fn visible(&self) -> impl Iterator<Item = (&'static str, &Change)> {
        let mcps = self.mcps.iter().map(|c| ("mcp", c));
        let skills = self.skills.iter().map(|c| ("skill", c));
        mcps.chain(skills).filter(|(_, c)| c.action != Action::Keep)
    }

    fn detail(&self, kind: &str, change: &Change) -> String {
        let spec = self.desired.mcps.get(&change.name);
        match (kind, spec) {
            ("mcp", Some(spec)) if change.action != Action::Conflict => {
                format!(" [{}]", spec.describe())
            }
            _ => String::new(),
        }
    }
}

/// Everything needed to show or apply the next sync.
pub struct Report {
    pub dirs: Dirs,
    pub machine: Machine,
    pub state: State,
    pub remote: String,
    pub snapshot: Snapshot,
    pub agents: Vec<AgentPlan>,
    pub skipped: Vec<String>,
}

impl Report {
    /// Resolves the remote commit, snapshots it and plans each agent.
    pub fn build(adopt: bool) -> anyhow::Result<Report> {
        let dirs = Dirs::from_env()?;
        let machine = Machine::load(&dirs.machine_file())?;
        let state = State::load(&dirs.state_file())?;
        let remote = git::resolve(&machine.repo, &machine.git_ref)?;
        let snapshot = Snapshot::fetch(&machine.repo, &machine.git_ref, &remote)?;
        let text = std::fs::read_to_string(snapshot.path().join("agsync.yaml"))
            .map_err(|e| anyhow::anyhow!("agsync.yaml nao encontrado no repo: {e}"))?;
        let manifest = Manifest::parse(&text)?;
        manifest.check_skill_dirs(snapshot.path())?;

        let mut skipped = skipped_git_items(&manifest, &machine.profiles);
        let mut agents = Vec::new();
        for agent in AGENTS {
            let adapter = match agent::detect(agent) {
                Ok(adapter) => adapter,
                Err(reason) => {
                    skipped.push(format!("{agent}: {reason}"));
                    continue;
                }
            };
            let desired = Desired::for_agent(&manifest, snapshot.path(), &machine.profiles, agent)?;
            let tracked = state.agent(agent);
            let mcps = plan(
                &desired.mcp_wants(),
                &keys(&tracked.mcps),
                &adapter.mcps()?,
                adopt,
            );
            let skills = plan(
                &desired.skill_wants(),
                &keys(&tracked.skills),
                &adapter.skills()?,
                adopt,
            );
            agents.push(AgentPlan {
                agent,
                adapter,
                desired,
                mcps,
                skills,
            });
        }
        Ok(Report {
            dirs,
            machine,
            state,
            remote,
            snapshot,
            agents,
            skipped,
        })
    }

    /// True if apply would change an agent config or the state.
    pub fn has_pending(&self) -> bool {
        self.agents
            .iter()
            .flat_map(|a| a.visible())
            .any(|(_, c)| c.action != Action::Conflict)
    }

    /// Prints changes per agent; env and header values are never shown.
    pub fn print_changes(&self) {
        for plan in &self.agents {
            println!("{}:", plan.agent);
            let mut any = false;
            for (kind, change) in plan.visible() {
                any = true;
                let detail = plan.detail(kind, change);
                println!(
                    "  {kind} {}: {}{detail}",
                    change.name,
                    change.action.label()
                );
            }
            if !any {
                println!("  nada a mudar");
            }
        }
    }

    pub fn print_skipped(&self) {
        if self.skipped.is_empty() {
            return;
        }
        println!("pulados:");
        for line in &self.skipped {
            println!("  {line}");
        }
    }
}

/// `agsync status`.
pub fn status() -> anyhow::Result<()> {
    let report = Report::build(false)?;
    let applied = report.state.commit.as_deref().unwrap_or("nenhum");
    println!("repo: {} ({})", report.machine.repo, report.machine.git_ref);
    println!("perfis ativos: {}", report.machine.profiles.join(", "));
    println!("commit aplicado: {applied}");
    println!("commit remoto:   {}", report.remote);
    report.print_changes();
    report.print_skipped();
    report.snapshot.close()
}

/// `agsync diff`.
pub fn diff() -> anyhow::Result<()> {
    let report = Report::build(false)?;
    report.print_changes();
    report.snapshot.close()
}
