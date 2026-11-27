//! `agsync rollback`: applies the commit applied before the current one, through the
//! normal plan and apply. Backups are never copied back over agent configs.

use crate::apply::{Applied, Event};
use crate::git::Snapshot;
use crate::lock::Lock;
use crate::paths::Dirs;
use crate::prompt::Confirm;
use crate::report::{Inputs, Report, read_manifest};

/// The commit a rollback goes back to, found under the lock and not planned yet.
pub struct Target {
    lock: Lock,
    inputs: Inputs,
    /// Commit applied now.
    pub from: String,
    /// Commit applied before it, the one the rollback applies.
    pub to: String,
}

/// [`Target`] planned: what the rollback would change, before anything is asked.
pub struct Planned {
    _lock: Lock,
    pub from: String,
    pub to: String,
    pub report: Report,
}

/// Takes the lock and finds the previous commit; an error when there is none.
pub fn target() -> anyhow::Result<Target> {
    let lock = Lock::wait(&Dirs::from_env()?)?;
    let inputs = Inputs::load()?;
    let Some(to) = inputs.state.previous.clone() else {
        anyhow::bail!(
            "sem commit anterior em {}: o rollback precisa de dois applies de commits diferentes",
            inputs.dirs.state_file().display()
        );
    };
    let from = inputs.state.commit.clone().unwrap_or_default();
    Ok(Target {
        lock,
        inputs,
        from,
        to,
    })
}

impl Target {
    /// Fetches the previous commit and plans it like any apply.
    pub fn plan(self) -> anyhow::Result<Planned> {
        let repo = self.inputs.machine.repo.clone();
        let cache = self.inputs.dirs.snapshot_cache();
        let snapshot = Snapshot::cached(&cache, &repo, &self.to, &self.to)?;
        let manifest = read_manifest(&snapshot)?;
        let report = Report::plan(self.inputs, self.to.clone(), snapshot, manifest, false)?;
        Ok(Planned {
            _lock: self.lock,
            from: self.from,
            to: self.to,
            report,
        })
    }
}

/// Asks and applies the planned commit, like [`crate::apply::run`]. `watch.json` is
/// deleted right after a yes, before anything is written, so a run that fails later
/// still leaves it gone; on no nothing changes.
pub fn run(
    planned: Planned,
    confirm: &dyn Confirm,
    on: &mut dyn FnMut(&Event),
) -> anyhow::Result<Applied> {
    let dirs = planned.report.dirs.clone();
    let forget = ForgetOnYes {
        confirm,
        dirs: &dirs,
    };
    let applied = crate::apply::run(planned.report, &forget, "agsync rollback", |_| Ok(()), on)?;
    if applied.confirmed {
        forget_watch(&dirs)?;
    }
    Ok(applied)
}

/// [`Confirm`] that deletes `watch.json` as soon as the answer is yes.
struct ForgetOnYes<'a> {
    confirm: &'a dyn Confirm,
    dirs: &'a Dirs,
}

impl Confirm for ForgetOnYes<'_> {
    fn can_answer(&self) -> bool {
        self.confirm.can_answer()
    }

    fn ask(&self, question: &str) -> anyhow::Result<bool> {
        let yes = self.confirm.ask(question)?;
        if yes {
            forget_watch(self.dirs)?;
        }
        Ok(yes)
    }
}

/// Without `watch.json` the next check plans again and reports the remote as pending.
fn forget_watch(dirs: &Dirs) -> anyhow::Result<()> {
    match std::fs::remove_file(dirs.watch_file()) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(anyhow::Error::new(e).context("apagar watch.json"))
        }
        _ => Ok(()),
    }
}
