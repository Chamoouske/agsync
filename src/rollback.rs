//! `agsync rollback`: applies the commit applied before the current one, through the
//! normal plan and apply. Backups are never copied back over agent configs.

use crate::git::Snapshot;
use crate::lock::Lock;
use crate::paths::Dirs;
use crate::report::{Inputs, Report, read_manifest};

/// Plans the previous commit, shows the diff, asks and applies it.
pub fn rollback(yes: bool) -> anyhow::Result<()> {
    let _lock = Lock::wait(&Dirs::from_env()?)?;
    let inputs = Inputs::load()?;
    let Some(previous) = inputs.state.previous.clone() else {
        anyhow::bail!(
            "sem commit anterior em {}: o rollback precisa de dois applies de commits diferentes",
            inputs.dirs.state_file().display()
        );
    };
    let current = inputs.state.commit.clone().unwrap_or_default();
    println!("rollback: {current} -> {previous}");
    let repo = inputs.machine.repo.clone();
    let cache = inputs.dirs.snapshot_cache();
    let snapshot = Snapshot::cached(&cache, &repo, &previous, &previous)?;
    let manifest = read_manifest(&snapshot)?;
    let report = Report::plan(inputs, previous.clone(), snapshot, manifest, false)?;
    forget_watch(&report.dirs)?;
    if !crate::apply::write(report, yes, "agsync rollback", |_| Ok(()))? {
        return Ok(());
    }
    println!("aplicado o commit anterior {previous}");
    println!(
        "o `agsync check` volta a acusar o commit remoto como pendente; isso e esperado. Para voltar a ele: `agsync apply`"
    );
    Ok(())
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
