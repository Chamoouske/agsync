//! `agsync init`: machine.toml, repo access, schedule, shell hook, first apply.
//!
//! Each step takes its options and returns what happened without reading stdin or
//! printing; [`run`] chains them and the CLI prints them. Each step
//! reports its own failure; later steps still run when they make sense.

use std::path::PathBuf;

use serde::Serialize;

use crate::git;
use crate::machine::Machine;
use crate::paths::Dirs;
use crate::prompt::Confirm;
use crate::schedule::Scheduled;
use crate::shell::Hooked;

/// The answers `init` needs, all given up front.
#[derive(Debug, Clone)]
pub struct Options {
    pub url: String,
    pub git_ref: Option<String>,
    pub profiles: Vec<String>,
    /// Applies without asking.
    pub yes: bool,
    /// `--silent-update`: applies without asking unless a build comes from an import.
    pub silent: bool,
}

/// Step 1: the machine.toml written.
#[derive(Serialize, Debug)]
pub struct Saved {
    pub file: PathBuf,
    pub machine: Machine,
}

/// Step 2: whether the config repo answers. Git never asks for a password here.
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Access {
    Reachable {
        commit: String,
    },
    Unreachable {
        reason: String,
        hint: Option<&'static str>,
    },
}

/// Step 3: the periodic check.
#[derive(Serialize, Debug)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Scheduling {
    Scheduled(Scheduled),
    Failed { reason: String },
}

/// Step 4: the shell hook.
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Hook {
    /// Nobody can answer the questions; nothing was installed.
    NoTty,
    Offered {
        shells: Vec<Hooked>,
    },
    Failed {
        reason: String,
    },
}

/// Step 5: the first apply.
#[derive(Serialize, Debug, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum FirstApply {
    Applied,
    /// Skipped because the repo is unreachable.
    Unreachable,
    /// Skipped because it would have to ask without a terminal.
    NoTty,
    Failed {
        reason: String,
    },
}

/// What every step did.
#[derive(Serialize, Debug)]
pub struct Outcome {
    pub saved: Saved,
    pub access: Access,
    pub schedule: Scheduling,
    pub hook: Hook,
    pub apply: FirstApply,
}

impl Outcome {
    /// False when the repo is unreachable, a hook failed or the apply failed.
    pub fn ok(&self) -> bool {
        let reachable = matches!(self.access, Access::Reachable { .. });
        reachable && self.hook.ok() && self.apply.ok()
    }
}

impl Hook {
    /// False when a profile could not get the hook.
    pub fn ok(&self) -> bool {
        match self {
            Hook::NoTty => true,
            Hook::Offered { shells } => !shells.iter().any(|h| matches!(h, Hooked::Failed { .. })),
            Hook::Failed { .. } => false,
        }
    }
}

impl FirstApply {
    /// False only when the apply ran and failed.
    pub fn ok(&self) -> bool {
        !matches!(self, FirstApply::Failed { .. })
    }
}

/// Runs every step with `options`, without reading stdin or printing in steps 1 to 4.
/// `confirm` answers the shell hook questions. Errs only when machine.toml cannot be
/// written. The first apply still prints its plan and results.
pub fn run(options: &Options, confirm: &dyn Confirm) -> anyhow::Result<Outcome> {
    let saved = save(&Dirs::from_env()?, options)?;
    let access = check_access(&saved.machine);
    let schedule = schedule();
    let hook = hook(confirm);
    let apply = first_apply(&access, options.yes, options.silent);
    Ok(Outcome {
        saved,
        access,
        schedule,
        hook,
        apply,
    })
}

/// Step 1: writes machine.toml in `dirs` from `options`.
pub fn save(dirs: &Dirs, options: &Options) -> anyhow::Result<Saved> {
    let machine = Machine::new(
        options.url.clone(),
        options.git_ref.clone(),
        options.profiles.clone(),
    );
    let file = dirs.machine_file();
    machine.save(&file)?;
    Ok(Saved { file, machine })
}

/// Step 2: resolves the configured ref on the repo.
pub fn check_access(machine: &Machine) -> Access {
    match git::resolve(&machine.repo, &machine.git_ref) {
        Ok(commit) => Access::Reachable { commit },
        Err(e) => {
            let reason = format!("{e:#}");
            let hint = crate::doctor::hint(&reason);
            Access::Unreachable { reason, hint }
        }
    }
}

/// Step 3: registers the periodic check.
pub fn schedule() -> Scheduling {
    match crate::schedule::register() {
        Ok(scheduled) => Scheduling::Scheduled(scheduled),
        Err(e) => Scheduling::Failed {
            reason: format!("{e:#}"),
        },
    }
}

/// Step 4: offers the hook to each shell, asking `confirm`.
pub fn hook(confirm: &dyn Confirm) -> Hook {
    if !confirm.can_answer() {
        return Hook::NoTty;
    }
    let mut shells = Vec::new();
    match crate::shell::offer(confirm, &mut |hooked| shells.push(hooked)) {
        Ok(()) => Hook::Offered { shells },
        Err(e) => Hook::Failed {
            reason: format!("{e:#}"),
        },
    }
}

/// Step 5: the first apply, skipped when the repo is unreachable.
pub fn first_apply(access: &Access, yes: bool, silent: bool) -> FirstApply {
    if !matches!(access, Access::Reachable { .. }) {
        return FirstApply::Unreachable;
    }
    match crate::apply::init_apply(yes, silent) {
        Ok(true) => FirstApply::Applied,
        Ok(false) => FirstApply::NoTty,
        Err(e) => FirstApply::Failed {
            reason: format!("{e:#}"),
        },
    }
}
