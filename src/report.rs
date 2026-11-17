//! Snapshot the config repo and plan every agent: shared by status, diff and apply.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::agent::{self, Adapter};
use crate::compose::{Origins, compose};
use crate::desired::{Context, Desired};
use crate::git::{self, Snapshot};
use crate::machine::Machine;
use crate::manifest::{Manifest, selected, targets};
use crate::paths::Dirs;
use crate::plan::{Action, Change, plan};
use crate::resolved::{Resolved, Watched};
use crate::secret::{self, Secrets};
use crate::source::BuildJob;
use crate::state::{AgentState, State, keys};

/// Plan for one agent that has an adapter on this machine.
pub struct AgentPlan {
    pub agent: &'static str,
    pub adapter: Box<dyn Adapter>,
    pub desired: Desired,
    pub mcps: Vec<Change>,
    pub skills: Vec<Change>,
    pub plugins: Vec<Change>,
    /// Id the agent already uses for each desired plugin: from the declared source, or
    /// the manual install of the same name that `--adopt` replaces.
    pub plugin_ids: BTreeMap<String, String>,
}

impl AgentPlan {
    /// Plans every MCP, skill and plugin of one agent. Each kind is planned against the
    /// agent's entries of that kind only, so a homonym of another kind never matches.
    pub fn new(
        agent: &'static str,
        adapter: Box<dyn Adapter>,
        desired: Desired,
        tracked: &AgentState,
        adopt: bool,
        skipped: &mut Vec<String>,
    ) -> anyhow::Result<AgentPlan> {
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
        let (plugins, plugin_ids) =
            plan_plugins(adapter.as_ref(), &desired, tracked, adopt, agent, skipped)?;
        Ok(AgentPlan {
            agent,
            adapter,
            desired,
            mcps,
            skills,
            plugins,
            plugin_ids,
        })
    }

    /// Changes worth showing (everything but `Keep`).
    pub fn visible(&self) -> impl Iterator<Item = (&'static str, &Change)> {
        let mcps = self.mcps.iter().map(|c| ("mcp", c));
        let skills = self.skills.iter().map(|c| ("skill", c));
        let plugins = self.plugins.iter().map(|c| ("plugin", c));
        mcps.chain(skills)
            .chain(plugins)
            .filter(|(_, c)| c.action != Action::Keep)
    }

    fn detail(&self, kind: &str, change: &Change) -> String {
        let spec = self.desired.mcps.get(&change.name);
        match (kind, spec) {
            ("mcp", Some(want)) if change.action != Action::Conflict => {
                format!(" [{}]", want.shown.describe())
            }
            ("plugin", _) if change.action != Action::Conflict => {
                match self.desired.plugins.get(&change.name) {
                    Some(plugin) => format!(" [{}]", plugin.describe()),
                    None => String::new(),
                }
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
    /// Git sources on a branch or tag, with the commit they resolved to.
    pub watched: Vec<Watched>,
    /// `silentUpdate` from agsync.yaml.
    pub silent_update: bool,
    /// Import each imported item came from; items of the main repo are not here.
    pub origins: Origins,
}

/// Local files every plan starts from.
pub struct Inputs {
    pub dirs: Dirs,
    pub machine: Machine,
    pub state: State,
    pub secrets: Secrets,
}

impl Inputs {
    pub fn load() -> anyhow::Result<Inputs> {
        let dirs = Dirs::from_env()?;
        let machine = Machine::load(&dirs.machine_file())?;
        let state = State::load(&dirs.state_file())?;
        let secrets = Secrets::load(&secret::file_path(&dirs.config))?;
        Ok(Inputs {
            dirs,
            machine,
            state,
            secrets,
        })
    }
}

/// Reads and validates `agsync.yaml` from the snapshot, local skill folders included.
pub fn read_manifest(snapshot: &Snapshot) -> anyhow::Result<Manifest> {
    let text = std::fs::read_to_string(snapshot.path().join("agsync.yaml"))
        .map_err(|e| anyhow::anyhow!("agsync.yaml nao encontrado no repo: {e}"))?;
    let manifest = Manifest::parse(&text)?;
    manifest.check_skill_dirs(snapshot.path())?;
    Ok(manifest)
}

impl Report {
    /// Resolves the remote commit, takes its snapshot from the cache (checked out on a
    /// miss) and plans each agent.
    pub fn build(adopt: bool) -> anyhow::Result<Report> {
        let inputs = Inputs::load()?;
        let remote = git::resolve(&inputs.machine.repo, &inputs.machine.git_ref)?;
        let snapshot = Snapshot::cached(
            &inputs.dirs.snapshot_cache(),
            &inputs.machine.repo,
            &inputs.machine.git_ref,
            &remote,
        )?;
        let manifest = read_manifest(&snapshot)?;
        Report::plan(inputs, remote, snapshot, manifest, adopt)
    }

    /// Plans each agent for a snapshot whose manifest is already read, with the imports
    /// of the active profiles merged in.
    pub fn plan(
        inputs: Inputs,
        remote: String,
        snapshot: Snapshot,
        manifest: Manifest,
        adopt: bool,
    ) -> anyhow::Result<Report> {
        let Inputs {
            dirs,
            machine,
            state,
            secrets,
        } = inputs;
        let composed = compose(manifest, &machine.profiles, &dirs.import_cache())?;
        let manifest = &composed.manifest;
        let sources = Resolved::new(manifest, &machine.profiles, &dirs);
        let mut skipped = sources.skipped.clone();
        skipped.extend(composed.skipped.iter().cloned());
        let ctx = Context {
            manifest,
            root: snapshot.path(),
            active: &machine.profiles,
            secrets: &secrets,
            sources: &sources,
        };
        let mut agents = Vec::new();
        for (agent, detect) in agent::AGENTS {
            let adapter = match detect() {
                Ok(adapter) => adapter,
                Err(reason) => {
                    skipped.push(format!("{agent}: {reason}"));
                    continue;
                }
            };
            let desired = Desired::for_agent(&ctx, agent, &|spot| adapter.keeps_ref(spot))?;
            skipped.extend(desired.skipped.iter().cloned());
            let tracked = state.agent(agent);
            let plan = AgentPlan::new(agent, adapter, desired, &tracked, adopt, &mut skipped)?;
            agents.push(plan);
        }
        let builds = needed_builds(&sources, &agents);
        let reach: Vec<SkillReach> = agents.iter().map(SkillReach::of).collect();
        let mut warnings = composed.warnings.clone();
        warnings.extend(seen_anyway(manifest, &machine.profiles, &reach));
        warnings.extend(duplicate_items(
            agents.iter().map(|p| (p.agent, &p.desired)),
        ));
        let mut watched = sources.watched.clone();
        watched.extend(composed.watched.iter().cloned());
        watched.sort();
        watched.dedup();
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
            watched,
            silent_update: manifest.silent_update,
            origins: composed.origins.clone(),
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

    /// True when some build apply would run comes from an imported MCP.
    pub fn has_imported_build(&self) -> bool {
        self.builds
            .iter()
            .any(|job| self.origins.contains_key(&("mcp", job.name.clone())))
    }

    /// Prints every build apply would run: third-party code on this machine.
    pub fn print_builds(&self) {
        if self.builds.is_empty() {
            return;
        }
        println!();
        println!("!!! BUILDS: estes comandos executam codigo de terceiros nesta maquina !!!");
        for job in &self.builds {
            let origin = origin_note(&self.origins, "mcp", &job.name);
            println!(
                "  mcp {}: {} @ {} ({}){origin}",
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
                let origin = origin_note(&self.origins, kind, &change.name);
                println!(
                    "  {kind} {}: {}{detail}{origin}",
                    change.name,
                    change.action.label()
                );
            }
            if !any {
                println!("  nada a mudar");
            }
        }
    }

    /// Lists manual installs that match a declared item by name, kind and agent: the
    /// ones apply replaces (`--adopt`) or the ones it leaves alone.
    pub fn print_manual(&self) {
        let pick = |action: Action| -> Vec<String> {
            self.agents
                .iter()
                .flat_map(|plan| {
                    plan.visible()
                        .filter(move |(_, c)| c.action == action)
                        .map(move |(kind, c)| {
                            let detail = plan.detail(kind, c);
                            format!("  {}: {kind} {}{detail}", plan.agent, c.name)
                        })
                })
                .collect()
        };
        let replaced = pick(Action::Adopt);
        if !replaced.is_empty() {
            println!();
            println!(
                "!!! INSTALACOES MANUAIS que serao substituidas pela definicao do agsync.yaml:"
            );
            replaced.iter().for_each(|line| println!("{line}"));
        }
        let kept = pick(Action::Conflict);
        if !kept.is_empty() {
            println!();
            println!(
                "instalacoes manuais com o mesmo nome no agsync.yaml (`agsync apply --adopt` substitui):"
            );
            kept.iter().for_each(|line| println!("{line}"));
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
    report.print_manual();
    report.print_builds();
    report.print_skipped();
    report.print_warnings();
    report.snapshot.close()
}

/// `agsync diff`.
pub fn diff() -> anyhow::Result<()> {
    let report = Report::build(false)?;
    report.print_changes();
    report.print_manual();
    report.print_builds();
    report.print_warnings();
    report.snapshot.close()
}

/// ` (de imports.<kind> <git>@<ref>)` for an imported item, empty for the main repo's.
fn origin_note(origins: &Origins, kind: &'static str, name: &str) -> String {
    origins
        .get(&(kind, name.to_string()))
        .map(|origin| format!(" (de {origin})"))
        .unwrap_or_default()
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

/// Plans plugins for one agent. An agent whose CLI has no non-interactive plugin
/// support is left out, with the reason in `skipped`.
fn plan_plugins(
    adapter: &dyn Adapter,
    desired: &Desired,
    tracked: &AgentState,
    adopt: bool,
    agent: &str,
    skipped: &mut Vec<String>,
) -> anyhow::Result<(Vec<Change>, BTreeMap<String, String>)> {
    if desired.plugins.is_empty() && tracked.plugins.is_empty() {
        return Ok((Vec::new(), BTreeMap::new()));
    }
    if let Err(reason) = adapter.plugin_support() {
        skipped.push(format!("{agent}: plugins: {reason}"));
        return Ok((Vec::new(), BTreeMap::new()));
    }
    let mut current = BTreeMap::new();
    let mut ids = BTreeMap::new();
    for (name, spec) in &desired.plugins {
        if let Some(id) = adapter.plugin_installed(spec)? {
            current.insert(name.clone(), spec.hash());
            ids.insert(name.clone(), id);
            continue;
        }
        let tracked_here = tracked.plugins.contains_key(name);
        if !tracked_here && let Some(id) = adapter.plugin_named(name)? {
            current.insert(name.clone(), id.clone());
            ids.insert(name.clone(), id);
        }
    }
    for (name, entry) in &tracked.plugins {
        if desired.plugins.contains_key(name) {
            continue;
        }
        if let Some(id) = &entry.plugin
            && adapter.plugin_present(id)?
        {
            current.insert(name.clone(), entry.hash.clone());
        }
    }
    let changes = plan(
        &desired.plugin_wants(),
        &keys(&tracked.plugins),
        &current,
        adopt,
    );
    Ok((changes, ids))
}

/// Names configured both as a plugin and as an MCP or skill for the same agent.
fn duplicate_items<'a>(agents: impl Iterator<Item = (&'static str, &'a Desired)>) -> Vec<String> {
    let mut lines = Vec::new();
    for (agent, desired) in agents {
        for (name, spec) in &desired.plugins {
            for (kind, present) in [
                ("mcps", desired.mcps.contains_key(name)),
                ("skills", desired.skills.contains_key(name)),
            ] {
                if present {
                    lines.push(format!(
                        "plugin {name} ({}) tambem esta em {kind} para {agent}; pode duplicar no agente",
                        spec.describe()
                    ));
                }
            }
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
    fn plugin_also_declared_as_mcp_or_skill_is_warned_per_agent() {
        use crate::deliver::McpWant;
        use crate::desired::SkillSrc;
        use crate::mcp::McpSpec;
        use crate::plugin::PluginSpec;

        let mut desired = Desired::default();
        for name in ["both", "mcp-only", "skill-only"] {
            let spec = PluginSpec::new(name, "https://x/market.git", "v1");
            desired.plugins.insert(name.to_string(), spec);
        }
        let mcp = McpSpec::Http {
            url: "https://x".into(),
            headers: BTreeMap::new(),
        };
        desired.mcps.insert(
            "both".into(),
            McpWant {
                shown: mcp.clone(),
                write: mcp,
                hash: "h".into(),
                sha: None,
            },
        );
        desired.mcps.insert(
            "mcp-only".into(),
            McpWant {
                shown: McpSpec::Http {
                    url: "https://y".into(),
                    headers: BTreeMap::new(),
                },
                write: McpSpec::Http {
                    url: "https://y".into(),
                    headers: BTreeMap::new(),
                },
                hash: "h2".into(),
                sha: None,
            },
        );
        desired.skills.insert(
            "both".into(),
            SkillSrc {
                dir: PathBuf::from("/tmp/skill"),
                hash: "h3".into(),
                sha: None,
            },
        );

        let lines = duplicate_items([("claude", &desired), ("codex", &desired)].into_iter());
        assert_eq!(lines.len(), 6, "{lines:?}");
        assert!(
            lines[0].contains("plugin both")
                && lines[0].contains("mcps")
                && lines[0].contains("claude"),
            "{lines:?}"
        );
        assert!(lines[1].contains("plugin both") && lines[1].contains("skills"));
        assert!(lines[2].contains("plugin mcp-only") && lines[2].contains("mcps"));
        assert!(lines[3].contains("plugin both") && lines[3].contains("codex"));
        assert!(lines.iter().all(|l| !l.contains("skill-only")));
    }

    #[test]
    fn imported_items_name_their_origin() {
        let mut origins = Origins::new();
        let from = "imports.mcps https://x/team.git@main".to_string();
        origins.insert(("mcp", "team".to_string()), from);
        assert_eq!(
            origin_note(&origins, "mcp", "team"),
            " (de imports.mcps https://x/team.git@main)"
        );
        assert_eq!(origin_note(&origins, "skill", "team"), "");
        assert_eq!(origin_note(&origins, "mcp", "local"), "");
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
