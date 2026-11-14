//! Snapshot the config repo and plan every agent: shared by status, diff and apply.

use std::path::{Path, PathBuf};

use crate::agent::{self, Adapter};
use crate::desired::{Context, Desired};
use crate::git::{self, Snapshot};
use crate::machine::Machine;
use crate::manifest::{AGENTS, Manifest, selected, targets};
use crate::paths::Dirs;
use crate::plan::{Action, Change, plan};
use crate::resolved::Resolved;
use crate::secret::{self, Secrets};
use crate::source::BuildJob;
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
            ("mcp", Some(want)) if change.action != Action::Conflict => {
                format!(" [{}]", want.shown.describe())
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
    /// Builds apply runs before writing: new or changed commits, not built yet.
    pub builds: Vec<BuildJob>,
    pub skipped: Vec<String>,
    /// Things apply does not change but the user should know.
    pub warnings: Vec<String>,
}

impl Report {
    /// Resolves the remote commit, snapshots it and plans each agent.
    pub fn build(adopt: bool) -> anyhow::Result<Report> {
        let dirs = Dirs::from_env()?;
        let machine = Machine::load(&dirs.machine_file())?;
        let state = State::load(&dirs.state_file())?;
        let secrets = Secrets::load(&secret::file_path(&dirs.config))?;
        let remote = git::resolve(&machine.repo, &machine.git_ref)?;
        let snapshot = Snapshot::fetch(&machine.repo, &machine.git_ref, &remote)?;
        let text = std::fs::read_to_string(snapshot.path().join("agsync.yaml"))
            .map_err(|e| anyhow::anyhow!("agsync.yaml nao encontrado no repo: {e}"))?;
        let manifest = Manifest::parse(&text)?;
        manifest.check_skill_dirs(snapshot.path())?;

        let sources = Resolved::new(&manifest, &machine.profiles, &dirs);
        let mut skipped = sources.skipped.clone();
        let ctx = Context {
            manifest: &manifest,
            root: snapshot.path(),
            active: &machine.profiles,
            secrets: &secrets,
            sources: &sources,
        };
        let mut agents = Vec::new();
        for agent in AGENTS {
            let adapter = match agent::detect(agent) {
                Ok(adapter) => adapter,
                Err(reason) => {
                    skipped.push(format!("{agent}: {reason}"));
                    continue;
                }
            };
            let desired = Desired::for_agent(&ctx, agent, &|spot| adapter.keeps_ref(spot))?;
            skipped.extend(desired.skipped.iter().cloned());
            let tracked = state.agent(agent);
            let mut mcps = plan(
                &desired.mcp_wants(),
                &keys(&tracked.mcps),
                &adapter.mcps()?,
                adopt,
            );
            mcps.retain(|c| !desired.held_mcps.contains(&c.name));
            let mut skills = plan(
                &desired.skill_wants(),
                &keys(&tracked.skills),
                &adapter.skills()?,
                adopt,
            );
            skills.retain(|c| !desired.held_skills.contains(&c.name));
            agents.push(AgentPlan {
                agent,
                adapter,
                desired,
                mcps,
                skills,
            });
        }
        let builds = needed_builds(&sources, &agents);
        let reach: Vec<SkillReach> = agents.iter().map(SkillReach::of).collect();
        let warnings = seen_anyway(&manifest, &machine.profiles, &reach);
        Ok(Report {
            dirs,
            machine,
            state,
            remote,
            snapshot,
            agents,
            builds,
            skipped,
            warnings,
        })
    }

    /// True if apply would build, change an agent config or the state.
    pub fn has_pending(&self) -> bool {
        let changes = self
            .agents
            .iter()
            .flat_map(|a| a.visible())
            .any(|(_, c)| c.action != Action::Conflict);
        changes || !self.builds.is_empty()
    }

    /// Prints every build apply would run: third-party code on this machine.
    pub fn print_builds(&self) {
        if self.builds.is_empty() {
            return;
        }
        println!();
        println!("!!! BUILDS: estes comandos executam codigo de terceiros nesta maquina !!!");
        for job in &self.builds {
            println!(
                "  mcp {}: {} @ {} ({})",
                job.name, job.repo, job.sha, job.git_ref
            );
            if job.commands.is_empty() {
                println!("    (so checkout, nenhum comando)");
            }
            for command in &job.commands {
                println!("    $ {command}");
            }
        }
        println!();
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

    pub fn print_warnings(&self) {
        for line in &self.warnings {
            println!("aviso: {line}");
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
    report.print_builds();
    report.print_skipped();
    report.print_warnings();
    report.snapshot.close()
}

/// `agsync diff`.
pub fn diff() -> anyhow::Result<()> {
    let report = Report::build(false)?;
    report.print_changes();
    report.print_builds();
    report.print_warnings();
    report.snapshot.close()
}

/// Where a detected agent writes skills and where it loads them from.
pub struct SkillReach {
    pub agent: &'static str,
    pub writes: PathBuf,
    pub reads: Vec<PathBuf>,
}

impl SkillReach {
    pub fn of(plan: &AgentPlan) -> SkillReach {
        SkillReach {
            agent: plan.agent,
            writes: plan.adapter.skills_dir(),
            reads: plan.adapter.skill_dirs_read(),
        }
    }
}

/// One line per selected skill and per detected agent left out of its `agents:` that
/// loads it anyway, because it reads the skills folder of an agent that gets the skill.
fn seen_anyway(manifest: &Manifest, active: &[String], reach: &[SkillReach]) -> Vec<String> {
    let mut lines = Vec::new();
    for skill in &manifest.skills {
        let skill = &skill.value;
        if skill.agents.is_none() || !selected(&skill.profiles, active) {
            continue;
        }
        let gets = |r: &&SkillReach| targets(&skill.agents, r.agent);
        for outsider in reach.iter().filter(|r| !gets(r)) {
            let sources: Vec<&str> = reach
                .iter()
                .filter(gets)
                .filter(|r| outsider.reads.iter().any(|dir| same_dir(dir, &r.writes)))
                .map(|r| r.agent)
                .collect();
            if sources.is_empty() {
                continue;
            }
            lines.push(format!(
                "skill {} fica visivel no {} mesmo fora de agents: ele le as skills de {}",
                skill.name(),
                outsider.agent,
                sources.join(" e ")
            ));
        }
    }
    lines
}

/// Same folder on disk when both exist, else the same path.
fn same_dir(a: &Path, b: &Path) -> bool {
    match (std::fs::canonicalize(a), std::fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// Builds of MCPs that some agent will use (anything but a conflict).
fn needed_builds(sources: &Resolved, agents: &[AgentPlan]) -> Vec<BuildJob> {
    let used = |name: &String| {
        agents.iter().any(|plan| {
            plan.mcps
                .iter()
                .any(|c| &c.name == name && c.action != Action::Conflict)
                && plan.desired.mcps.contains_key(name)
        })
    };
    sources
        .mcps
        .iter()
        .filter(|(name, _)| used(name))
        .filter_map(|(_, src)| src.job.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = r#"
version: 1
profiles: [work]
skills:
  - path: skills/all
  - path: skills/claude-only
    agents: [claude]
  - path: skills/codex-and-opencode
    agents: [codex, opencode]
  - path: skills/both
    agents: [claude, codex]
  - path: skills/hermes-only
    agents: [hermes]
  - path: skills/inactive
    profiles: [work]
    agents: [codex]
"#;

    fn reach(agent: &'static str, writes: &str, reads: &[&str]) -> SkillReach {
        let mut all = vec![PathBuf::from(writes)];
        all.extend(reads.iter().map(PathBuf::from));
        SkillReach {
            agent,
            writes: PathBuf::from(writes),
            reads: all,
        }
    }

    fn agents(claude_skills: &str, names: &[&str]) -> Vec<SkillReach> {
        let all = [
            reach("claude", claude_skills, &[]),
            reach("codex", "/home/u/.agents/skills", &[]),
            reach(
                "opencode",
                "/home/u/.config/opencode/skills",
                &["/home/u/.claude/skills", "/home/u/.agents/skills"],
            ),
            reach("hermes", "/home/u/hermes/skills", &[]),
        ];
        all.into_iter()
            .filter(|r| names.contains(&r.agent))
            .collect()
    }

    #[test]
    fn warns_once_per_skill_opencode_sees_anyway() {
        let m = Manifest::parse(YAML).unwrap();
        let claude = "/home/u/.claude/skills";
        let all = agents(claude, &["claude", "codex", "opencode", "hermes"]);
        let lines = seen_anyway(&m, &[], &all);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].starts_with("skill claude-only fica visivel no opencode"));
        assert!(lines[0].ends_with("de claude"));
        assert!(lines[1].ends_with("de claude e codex"));

        let work = seen_anyway(&m, &["work".to_string()], &all);
        assert!(work.iter().any(|l| l.contains("skill inactive")));
        let without_opencode = agents(claude, &["claude", "codex"]);
        assert!(seen_anyway(&m, &[], &without_opencode).is_empty());
        let only_codex = seen_anyway(&m, &[], &agents(claude, &["codex", "opencode"]));
        assert_eq!(only_codex.len(), 1);
        assert!(only_codex[0].contains("skill both"));
    }

    #[test]
    fn claude_config_dir_elsewhere_is_not_seen_by_opencode() {
        let m = Manifest::parse(YAML).unwrap();
        let moved = agents(
            "/work/claude-config/skills",
            &["claude", "codex", "opencode"],
        );
        let lines = seen_anyway(&m, &[], &moved);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].starts_with("skill both fica visivel no opencode"));
        assert!(lines[0].ends_with("de codex"));
    }
}
