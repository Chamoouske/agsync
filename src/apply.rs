//! `agsync apply`: write planned changes, one item at a time.

use std::io::{BufRead, IsTerminal, Write};

use crate::agent::link;
use crate::paths::Dirs;
use crate::plan::{Action, Change};
use crate::report::{AgentPlan, Report};
use crate::state::{State, Tracked};

/// Shows the diff, asks for confirmation, applies, updates state, deletes the snapshot.
pub fn apply(yes: bool, adopt: bool) -> anyhow::Result<()> {
    let report = Report::build(adopt)?;
    report.print_changes();
    report.print_skipped();
    let mut state = report.state.clone();
    let pending = report.has_pending();

    if pending && !confirmed(yes)? {
        return report.snapshot.close();
    }

    let mut failures = 0;
    let mut mcp_written = false;
    for plan in &report.agents {
        let outcome = apply_agent(plan, &report.dirs, &mut state);
        failures += outcome.failures;
        mcp_written |= outcome.mcp_written;
    }
    state.commit = Some(report.snapshot.commit.clone());
    state.save(&report.dirs.state_file())?;
    report.snapshot.close()?;

    if !pending {
        println!("nada a aplicar");
    }
    if mcp_written {
        println!("Mudancas de MCP so valem apos reiniciar o agente.");
    }
    if failures > 0 {
        anyhow::bail!("{failures} item(ns) falharam; os demais foram aplicados");
    }
    Ok(())
}

fn confirmed(yes: bool) -> anyhow::Result<bool> {
    if yes {
        return Ok(true);
    }
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("sem TTY: nada aplicado; rode `agsync apply --yes`");
    }
    print!("Aplicar estas mudancas? [s/N] ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer)?;
    let ok = matches!(
        answer.trim().to_lowercase().as_str(),
        "s" | "sim" | "y" | "yes"
    );
    if !ok {
        println!("cancelado");
    }
    Ok(ok)
}

#[derive(Default)]
struct Outcome {
    failures: usize,
    mcp_written: bool,
}

fn apply_agent(plan: &AgentPlan, dirs: &Dirs, state: &mut State) -> Outcome {
    let mut outcome = Outcome::default();
    let writes = plan
        .mcps
        .iter()
        .chain(&plan.skills)
        .any(|c| c.action.writes());
    if writes && let Err(e) = plan.adapter.backup(&dirs.backups()) {
        eprintln!("{}: backup falhou, agente pulado: {e:#}", plan.agent);
        outcome.failures += 1;
        return outcome;
    }
    for change in &plan.mcps {
        match apply_mcp(plan, change, state) {
            Ok(()) => outcome.mcp_written |= change.action.writes(),
            Err(e) => {
                eprintln!("{}: mcp {} falhou: {e:#}", plan.agent, change.name);
                outcome.failures += 1;
            }
        }
        save(state, dirs);
    }
    for change in &plan.skills {
        if let Err(e) = apply_skill(plan, change, dirs, state) {
            eprintln!("{}: skill {} falhou: {e:#}", plan.agent, change.name);
            outcome.failures += 1;
        }
        save(state, dirs);
    }
    outcome
}

fn apply_mcp(plan: &AgentPlan, change: &Change, state: &mut State) -> anyhow::Result<()> {
    let name = &change.name;
    let desired = plan.desired.mcps.get(name);
    match (change.action, desired) {
        (Action::Add | Action::Update | Action::Overwrite | Action::Adopt, Some(spec)) => {
            plan.adapter.write_mcp(name, spec)?;
            track(state, plan.agent, "mcp", name, spec.hash(), None);
            report(plan, change, "mcp");
        }
        (Action::Remove, _) => {
            plan.adapter.remove_mcp(name)?;
            state.agent_mut(plan.agent).mcps.remove(name);
            report(plan, change, "mcp");
        }
        (Action::Forget, _) => {
            state.agent_mut(plan.agent).mcps.remove(name);
        }
        (Action::Keep, Some(spec)) => {
            track(state, plan.agent, "mcp", name, spec.hash(), None);
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
    match (change.action, desired) {
        (Action::Add | Action::Update | Action::Overwrite | Action::Adopt, Some(src)) => {
            let cached =
                link::ensure_cache(&src.dir, &dirs.skill_cache(), name, &src.hash, &backups)?;
            let mode = plan.adapter.write_skill(name, &cached, &backups)?;
            track(
                state,
                plan.agent,
                "skill",
                name,
                src.hash.clone(),
                Some(mode),
            );
            report(plan, change, "skill");
        }
        (Action::Remove, _) => {
            plan.adapter.remove_skill(name, &backups)?;
            state.agent_mut(plan.agent).skills.remove(name);
            report(plan, change, "skill");
        }
        (Action::Forget, _) => {
            state.agent_mut(plan.agent).skills.remove(name);
        }
        (Action::Keep, Some(src)) => {
            let mode = state
                .agent(plan.agent)
                .skills
                .get(name)
                .and_then(|t| t.mode);
            track(state, plan.agent, "skill", name, src.hash.clone(), mode);
        }
        _ => {}
    }
    Ok(())
}

fn track(
    state: &mut State,
    agent: &str,
    kind: &str,
    name: &str,
    hash: String,
    mode: Option<crate::state::LinkMode>,
) {
    let entries = state.agent_mut(agent);
    let map = if kind == "mcp" {
        &mut entries.mcps
    } else {
        &mut entries.skills
    };
    map.insert(
        name.to_string(),
        Tracked {
            hash,
            mode,
            sha: None,
        },
    );
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
