//! `agsync apply`: write planned changes, one item at a time.

use std::collections::BTreeSet;
use std::io::IsTerminal;

use crate::agent::link;
use crate::cleanup;
use crate::lock::Lock;
use crate::paths::Dirs;
use crate::pending::Pending;
use crate::plan::{Action, Change};
use crate::report::{AgentPlan, Report};
use crate::resolved::Watched;
use crate::state::State;
use crate::watch::Watch;

/// Shows the diff, asks for confirmation, applies, updates state, deletes the snapshot.
pub fn apply(yes: bool, adopt: bool) -> anyhow::Result<()> {
    let _lock = Lock::wait(&Dirs::from_env()?)?;
    let report = Report::build(adopt)?;
    report.print_changes();
    report.print_builds();
    report.print_skipped();
    report.print_warnings();
    let mut state = report.state.clone();
    let pending = report.has_pending();

    if pending && !confirmed(yes)? {
        return report.snapshot.close();
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
    report.snapshot.close()?;
    if failures == 0 {
        clean_caches(&report.dirs, &state);
        settle_check(&report.dirs, &report.remote, &report.watched)?;
    }

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
    Ok(())
}

/// After a clean apply nothing is pending, and `check` may stop early at this commit.
fn settle_check(dirs: &Dirs, remote: &str, watched: &[Watched]) -> anyhow::Result<()> {
    Pending::clear(&dirs.pending_file())?;
    let watch = Watch {
        commit: remote.to_string(),
        sources: watched.to_vec(),
    };
    watch.save(&dirs.watch_file())
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

fn confirmed(yes: bool) -> anyhow::Result<bool> {
    if yes {
        return Ok(true);
    }
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("sem TTY: nada aplicado; rode `agsync apply --yes`");
    }
    let ok = crate::prompt::yes_no("Aplicar estas mudancas?")?;
    if !ok {
        println!("cancelado");
    }
    Ok(ok)
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
