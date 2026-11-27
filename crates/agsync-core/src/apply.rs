//! `agsync apply`: write planned changes, one item at a time.

use std::collections::BTreeSet;

use crate::agent::link;
use crate::cleanup;
use crate::lock::Lock;
use crate::paths::Dirs;
use crate::pending::Pending;
use crate::plan::{Action, Change};
use crate::prompt::Confirm;
use crate::report::{AgentPlan, DiffView, Report};
use crate::state::State;
use crate::watch::Watch;

mod outcome;
mod print;

pub use outcome::{Applied, Event};

/// What a clean write does last: `apply` settles the check, `rollback` does not.
pub type AfterClean = fn(&Report) -> anyhow::Result<()>;

/// `agsync apply`: plans the remote head and writes it.
pub fn apply(confirm: &dyn Confirm, adopt: bool) -> anyhow::Result<()> {
    let _lock = Lock::wait(&Dirs::from_env()?)?;
    let report = Report::build(adopt)?;
    write(report, confirm, "agsync apply", settle_check).map(|_| ())
}

/// `agsync init`'s last step: [`apply`] that does not ask when `silent` is set or
/// agsync.yaml has `silentUpdate: true`, unless a build comes from an import. Returns
/// false when it would have to ask and `asker` cannot answer; nothing is written then.
pub fn init_apply(asker: &dyn Confirm, yes: bool, silent: bool) -> anyhow::Result<bool> {
    let _lock = Lock::wait(&Dirs::from_env()?)?;
    let no_tty = !asker.can_answer();
    let skip_without_asking = !yes && !silent && no_tty;
    let report = match Report::build(false) {
        Err(_) if skip_without_asking => return Ok(false),
        built => built?,
    };
    let (skip_prompt, note) = init_confirm(
        yes,
        silent,
        report.silent_update,
        report.has_imported_build(),
    );
    if let Some(note) = note {
        println!("{note}");
    }
    if !skip_prompt && no_tty {
        report.snapshot.close()?;
        return Ok(false);
    }
    let confirm = crate::prompt::yes_or(skip_prompt, asker);
    write(report, confirm, "agsync apply", settle_check)?;
    Ok(true)
}

/// Whether `init` applies without asking, and what to tell about it. `--yes` always
/// skips the prompt; `--silent-update` and `silentUpdate` do not when a build comes
/// from an imported repo, since that runs third-party code on this machine.
fn init_confirm(
    yes: bool,
    silent: bool,
    silent_update: bool,
    imported_build: bool,
) -> (bool, Option<&'static str>) {
    if yes {
        return (true, None);
    }
    if (silent || silent_update) && imported_build {
        let note = "build de mcp importado: a atualizacao silenciosa nao vale, confirme o apply";
        return (false, Some(note));
    }
    if silent {
        return (
            true,
            Some("atualizacao silenciosa: aplicando sem perguntar"),
        );
    }
    if silent_update {
        return (
            true,
            Some("silentUpdate no agsync.yaml: aplicando sem perguntar"),
        );
    }
    (false, None)
}

/// Shows the diff, asks for confirmation, applies, updates state, deletes the snapshot.
/// Returns false when the user said no; `command` is named when `--yes` is missing.
/// The caller holds the lock.
pub fn write(
    report: Report,
    confirm: &dyn Confirm,
    command: &str,
    after_clean: AfterClean,
) -> anyhow::Result<bool> {
    report.print_plan();
    let applied = run(report, confirm, command, after_clean, &mut |event| {
        event.print()
    })?;
    applied.finish()?;
    Ok(applied.confirmed)
}

/// [`write`] without printing: asks when something is pending, then builds, writes each
/// agent, updates state and deletes the snapshot. Nothing is built or written on a no.
/// Each step goes to `on` as it happens. The caller holds the lock.
pub fn run(
    report: Report,
    confirm: &dyn Confirm,
    command: &str,
    after_clean: AfterClean,
    on: &mut dyn FnMut(&Event),
) -> anyhow::Result<Applied> {
    let pending = report.has_pending();
    let mut applied = Applied {
        plan: DiffView::of(&report),
        skipped: report.skipped.clone(),
        imported_build: report.has_imported_build(),
        pending,
        confirmed: true,
        events: Vec::new(),
        failures: 0,
        mcp_written: false,
        hints: Vec::new(),
    };
    let question = "Aplicar estas mudancas?";
    if pending && !crate::prompt::answer(confirm, question, command)? {
        report.snapshot.close()?;
        applied.confirmed = false;
        return Ok(applied);
    }
    if pending {
        crate::secret::file::private_dir(&report.dirs.backups())?;
    }

    let mut events = Vec::new();
    let mut emit = |event: Event| {
        on(&event);
        events.push(event);
    };
    let mut state = report.state.clone();
    let failed_builds = run_builds(&report, &mut emit);
    applied.failures = failed_builds.len();
    for plan in &report.agents {
        let outcome = apply_agent(plan, &report.dirs, &mut state, &failed_builds, &mut emit);
        applied.failures += outcome.failures;
        applied.mcp_written |= outcome.mcp_written;
        if outcome.wrote {
            applied
                .hints
                .extend(plan.adapter.reload_hint().map(String::from));
        }
    }
    state.applied(&report.snapshot.commit);
    state.save(&report.dirs.state_file())?;
    if applied.failures == 0 {
        clean_caches(&report.dirs, &state, &mut emit);
        after_clean(&report)?;
    }
    report.snapshot.close()?;
    applied.events = events;
    Ok(applied)
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
fn clean_caches(dirs: &Dirs, state: &State, emit: &mut dyn FnMut(Event)) {
    let sources = cleanup::clean(&dirs.sources(), &cleanup::used_sources(state));
    let skills = cleanup::clean(&dirs.skill_cache(), &cleanup::used_skill_cache(state));
    match (sources, skills) {
        (Ok(a), Ok(b)) if a.len() + b.len() > 0 => emit(Event::Cleaned {
            count: a.len() + b.len(),
        }),
        (Err(e), _) | (_, Err(e)) => emit(Event::CleanupFailed {
            error: format!("{e:#}"),
        }),
        _ => {}
    }
}

/// Runs each build; returns the names of the MCPs whose build failed (they are skipped).
fn run_builds(report: &Report, emit: &mut dyn FnMut(Event)) -> BTreeSet<String> {
    let mut failed = BTreeSet::new();
    for job in &report.builds {
        let name = job.name.clone();
        emit(Event::BuildStarted {
            name: name.clone(),
            sha: job.sha.clone(),
        });
        match job.run(&report.dirs.logs()) {
            Ok(()) => emit(Event::BuildDone { name }),
            Err(e) => {
                failed.insert(name.clone());
                emit(Event::BuildFailed {
                    name,
                    error: format!("{e:#}"),
                });
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
    emit: &mut dyn FnMut(Event),
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
        emit(Event::BackupFailed {
            agent: plan.agent,
            error: format!("{e:#}"),
        });
        outcome.failures += 1;
        return outcome;
    }
    for change in mcps {
        match apply_mcp(plan, change, state, emit) {
            Ok(()) => {
                outcome.mcp_written |= change.action.writes();
                outcome.wrote |= change.action.writes();
            }
            Err(e) => {
                emit(failed(plan, "mcp", change, &e));
                outcome.failures += 1;
            }
        }
        save(state, dirs, emit);
    }
    for change in &plan.skills {
        match apply_skill(plan, change, dirs, state, emit) {
            Ok(()) => outcome.wrote |= change.action.writes(),
            Err(e) => {
                emit(failed(plan, "skill", change, &e));
                outcome.failures += 1;
            }
        }
        save(state, dirs, emit);
    }
    for change in &plan.plugins {
        let managed = managed_plugins(plan, state);
        match apply_plugin(plan, change, state, &managed, emit) {
            Ok(()) => outcome.wrote |= change.action.writes(),
            Err(e) => {
                emit(failed(plan, "plugin", change, &e));
                outcome.failures += 1;
            }
        }
        save(state, dirs, emit);
    }
    outcome
}

fn apply_mcp(
    plan: &AgentPlan,
    change: &Change,
    state: &mut State,
    emit: &mut dyn FnMut(Event),
) -> anyhow::Result<()> {
    let name = &change.name;
    let desired = plan.desired.mcps.get(name);
    let entries = state.agent_mut(plan.agent);
    match (change.action, desired) {
        (Action::Add | Action::Update | Action::Overwrite | Action::Adopt, Some(want)) => {
            plan.adapter.write_mcp(name, &want.write)?;
            entries.mcps.insert(name.clone(), want.tracked());
            emit(written(plan, "mcp", change));
        }
        (Action::Remove, _) => {
            plan.adapter.remove_mcp(name)?;
            entries.mcps.remove(name);
            emit(written(plan, "mcp", change));
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
    emit: &mut dyn FnMut(Event),
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
            emit(written(plan, "skill", change));
        }
        (Action::Remove, _) => {
            plan.adapter.remove_skill(name, &backups)?;
            entries.skills.remove(name);
            emit(written(plan, "skill", change));
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
    emit: &mut dyn FnMut(Event),
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
            emit(written(plan, "plugin", change));
        }
        (Action::Remove, _) => {
            if let Some(id) = entries.plugins.get(name).and_then(|t| t.plugin.clone()) {
                plan.adapter.uninstall_plugin(&id)?;
            }
            entries.plugins.remove(name);
            emit(written(plan, "plugin", change));
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

fn written(plan: &AgentPlan, kind: &'static str, change: &Change) -> Event {
    Event::Written {
        agent: plan.agent,
        kind,
        name: change.name.clone(),
        action: change.action,
    }
}

fn failed(plan: &AgentPlan, kind: &'static str, change: &Change, e: &anyhow::Error) -> Event {
    Event::ItemFailed {
        agent: plan.agent,
        kind,
        name: change.name.clone(),
        error: format!("{e:#}"),
    }
}

fn save(state: &State, dirs: &Dirs, emit: &mut dyn FnMut(Event)) {
    if let Err(e) = state.save(&dirs.state_file()) {
        emit(Event::StateSaveFailed {
            error: format!("{e:#}"),
        });
    }
}

#[cfg(test)]
mod tests;
