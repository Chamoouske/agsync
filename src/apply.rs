//! `agsync apply`: write planned changes, one item at a time.

use std::collections::BTreeSet;

use crate::agent::link;
use crate::cleanup;
use crate::lock::Lock;
use crate::paths::Dirs;
use crate::pending::Pending;
use crate::plan::{Action, Change};
use crate::report::{AgentPlan, Report};
use crate::state::State;
use crate::watch::Watch;

/// What a clean write does last: `apply` settles the check, `rollback` does not.
pub type AfterClean = fn(&Report) -> anyhow::Result<()>;

/// `agsync apply`: plans the remote head and writes it.
pub fn apply(yes: bool, adopt: bool) -> anyhow::Result<()> {
    let _lock = Lock::wait(&Dirs::from_env()?)?;
    let report = Report::build(adopt)?;
    write(report, yes, "agsync apply", settle_check).map(|_| ())
}

/// Shows the diff, asks for confirmation, applies, updates state, deletes the snapshot.
/// Returns false when the user said no; `command` is named when `--yes` is missing.
/// The caller holds the lock.
pub fn write(
    report: Report,
    yes: bool,
    command: &str,
    after_clean: AfterClean,
) -> anyhow::Result<bool> {
    report.print_changes();
    report.print_manual();
    report.print_builds();
    report.print_skipped();
    report.print_warnings();
    let mut state = report.state.clone();
    let pending = report.has_pending();

    let question = "Aplicar estas mudancas?";
    if pending && !crate::prompt::confirm(yes, question, command)? {
        report.snapshot.close()?;
        return Ok(false);
    }
    if pending {
        crate::secret::file::private_dir(&report.dirs.backups())?;
    }

    let failed_builds = run_builds(&report);
    let mut failures = failed_builds.len();
    let mut mcp_written = false;
    let mut hints = Vec::new();
    for plan in &report.agents {
        let outcome = apply_agent(plan, &report.dirs, &mut state, &failed_builds);
        failures += outcome.failures;
        mcp_written |= outcome.mcp_written;
        if outcome.wrote {
            hints.extend(plan.adapter.reload_hint());
        }
    }
    state.applied(&report.snapshot.commit);
    state.save(&report.dirs.state_file())?;
    if failures == 0 {
        clean_caches(&report.dirs, &state);
        after_clean(&report)?;
    }
    report.snapshot.close()?;

    if !pending {
        println!("nada a aplicar");
    }
    if mcp_written {
        println!("Mudancas de MCP so valem apos reiniciar o agente.");
    }
    for hint in hints {
        println!("{hint}");
    }
    if failures > 0 {
        anyhow::bail!("{failures} item(ns) falharam; os demais foram aplicados");
    }
    Ok(true)
}

/// After a clean apply nothing is pending, and `check` may stop early at this commit.
fn settle_check(report: &Report) -> anyhow::Result<()> {
    Pending::clear(&report.dirs.pending_file())?;
    let watch = Watch {
        commit: report.remote.clone(),
        sources: report.watched.clone(),
    };
    watch.save(&report.dirs.watch_file())
}

/// Deletes source checkouts and cached skills that no state entry uses.
fn clean_caches(dirs: &Dirs, state: &State) {
    let sources = cleanup::clean(&dirs.sources(), &cleanup::used_sources(state));
    let skills = cleanup::clean(&dirs.skill_cache(), &cleanup::used_skill_cache(state));
    match (sources, skills) {
        (Ok(a), Ok(b)) if a.len() + b.len() > 0 => {
            println!(
                "limpeza: {} pasta(s) sem uso removida(s)",
                a.len() + b.len()
            );
        }
        (Err(e), _) | (_, Err(e)) => eprintln!("aviso: limpeza do cache falhou: {e:#}"),
        _ => {}
    }
}

/// Runs each build; returns the names of the MCPs whose build failed (they are skipped).
fn run_builds(report: &Report) -> BTreeSet<String> {
    let mut failed = BTreeSet::new();
    for job in &report.builds {
        println!("build mcp {} ({})...", job.name, job.sha);
        match job.run(&report.dirs.logs()) {
            Ok(()) => println!("build mcp {}: ok", job.name),
            Err(e) => {
                eprintln!("mcp {}: {e:#}; item pulado", job.name);
                failed.insert(job.name.clone());
            }
        }
    }
    failed
}

#[derive(Default)]
struct Outcome {
    failures: usize,
    mcp_written: bool,
    /// Some MCP or skill of this agent was written.
    wrote: bool,
}

fn apply_agent(
    plan: &AgentPlan,
    dirs: &Dirs,
    state: &mut State,
    failed_builds: &BTreeSet<String>,
) -> Outcome {
    let mut outcome = Outcome::default();
    let mcps: Vec<&Change> = plan
        .mcps
        .iter()
        .filter(|c| !failed_builds.contains(&c.name))
        .collect();
    let writes = mcps
        .iter()
        .copied()
        .chain(&plan.skills)
        .chain(&plan.plugins)
        .any(|c| c.action.writes());
    if writes && let Err(e) = plan.adapter.backup(&dirs.backups()) {
        eprintln!("{}: backup falhou, agente pulado: {e:#}", plan.agent);
        outcome.failures += 1;
        return outcome;
    }
    for change in mcps {
        match apply_mcp(plan, change, state) {
            Ok(()) => {
                outcome.mcp_written |= change.action.writes();
                outcome.wrote |= change.action.writes();
            }
            Err(e) => {
                eprintln!("{}: mcp {} falhou: {e:#}", plan.agent, change.name);
                outcome.failures += 1;
            }
        }
        save(state, dirs);
    }
    for change in &plan.skills {
        match apply_skill(plan, change, dirs, state) {
            Ok(()) => outcome.wrote |= change.action.writes(),
            Err(e) => {
                eprintln!("{}: skill {} falhou: {e:#}", plan.agent, change.name);
                outcome.failures += 1;
            }
        }
        save(state, dirs);
    }
    for change in &plan.plugins {
        let managed = managed_plugins(plan, state);
        match apply_plugin(plan, change, state, &managed) {
            Ok(()) => outcome.wrote |= change.action.writes(),
            Err(e) => {
                eprintln!("{}: plugin {} falhou: {e:#}", plan.agent, change.name);
                outcome.failures += 1;
            }
        }
        save(state, dirs);
    }
    outcome
}

fn apply_mcp(plan: &AgentPlan, change: &Change, state: &mut State) -> anyhow::Result<()> {
    let name = &change.name;
    let desired = plan.desired.mcps.get(name);
    let entries = state.agent_mut(plan.agent);
    match (change.action, desired) {
        (Action::Add | Action::Update | Action::Overwrite | Action::Adopt, Some(want)) => {
            plan.adapter.write_mcp(name, &want.write)?;
            entries.mcps.insert(name.clone(), want.tracked());
            report(plan, change, "mcp");
        }
        (Action::Remove, _) => {
            plan.adapter.remove_mcp(name)?;
            entries.mcps.remove(name);
            report(plan, change, "mcp");
        }
        (Action::Forget, _) => {
            entries.mcps.remove(name);
        }
        (Action::Keep, Some(want)) => {
            entries.mcps.insert(name.clone(), want.tracked());
        }
        _ => {}
    }
    Ok(())
}

fn apply_skill(
    plan: &AgentPlan,
    change: &Change,
    dirs: &Dirs,
    state: &mut State,
) -> anyhow::Result<()> {
    let name = &change.name;
    let backups = dirs.backups();
    let desired = plan.desired.skills.get(name);
    let entries = state.agent_mut(plan.agent);
    match (change.action, desired) {
        (Action::Add | Action::Update | Action::Overwrite | Action::Adopt, Some(src)) => {
            let cached =
                link::ensure_cache(&src.dir, &dirs.skill_cache(), name, &src.hash, &backups)?;
            let mode = plan.adapter.write_skill(name, &cached, &backups)?;
            entries.skills.insert(name.clone(), src.tracked(Some(mode)));
            report(plan, change, "skill");
        }
        (Action::Remove, _) => {
            plan.adapter.remove_skill(name, &backups)?;
            entries.skills.remove(name);
            report(plan, change, "skill");
        }
        (Action::Forget, _) => {
            entries.skills.remove(name);
        }
        (Action::Keep, Some(src)) => {
            let mode = entries.skills.get(name).and_then(|t| t.mode);
            entries.skills.insert(name.clone(), src.tracked(mode));
        }
        _ => {}
    }
    Ok(())
}

/// Plugin ids agsync tracks for this agent right now, including ones added earlier in
/// the same apply.
fn managed_plugins(plan: &AgentPlan, state: &State) -> BTreeSet<String> {
    state
        .agent(plan.agent)
        .plugins
        .values()
        .filter_map(|t| t.plugin.clone())
        .collect()
}

fn apply_plugin(
    plan: &AgentPlan,
    change: &Change,
    state: &mut State,
    managed: &BTreeSet<String>,
) -> anyhow::Result<()> {
    let name = &change.name;
    let desired = plan.desired.plugins.get(name);
    let entries = state.agent_mut(plan.agent);
    match (change.action, desired) {
        (Action::Add | Action::Update | Action::Overwrite | Action::Adopt, Some(spec)) => {
            let previous = match entries.plugins.get(name) {
                Some(tracked) => tracked.plugin.clone(),
                None => plan.plugin_ids.get(name).cloned(),
            };
            let id = plan.adapter.install_plugin(spec, managed)?;
            if let Some(old) = previous.filter(|old| old != &id) {
                plan.adapter.uninstall_plugin(&old)?;
            }
            entries.plugins.insert(name.clone(), spec.tracked(id));
            report(plan, change, "plugin");
        }
        (Action::Remove, _) => {
            if let Some(id) = entries.plugins.get(name).and_then(|t| t.plugin.clone()) {
                plan.adapter.uninstall_plugin(&id)?;
            }
            entries.plugins.remove(name);
            report(plan, change, "plugin");
        }
        (Action::Forget, _) => {
            entries.plugins.remove(name);
        }
        (Action::Keep, Some(spec)) => {
            let id = match entries.plugins.get(name).and_then(|t| t.plugin.clone()) {
                Some(id) => Some(id),
                None => plan
                    .plugin_ids
                    .get(name)
                    .cloned()
                    .or(plan.adapter.plugin_installed(spec)?),
            };
            if let Some(id) = id {
                entries.plugins.insert(name.clone(), spec.tracked(id));
            }
        }
        _ => {}
    }
    Ok(())
}

fn report(plan: &AgentPlan, change: &Change, kind: &str) {
    println!(
        "{}: {kind} {}: ok ({})",
        plan.agent,
        change.name,
        change.action.label()
    );
}

fn save(state: &State, dirs: &Dirs) {
    if let Err(e) = state.save(&dirs.state_file()) {
        eprintln!("gravar state.json falhou: {e:#}");
    }
}

