//! `agsync check`: reads the remote only, records news in `pending.json` and notifies
//! once per news. Never prompts and never applies anything.

use std::path::Path;
use std::time::SystemTime;

use crate::checklog;
use crate::git::{self, Snapshot};
use crate::lock::Lock;
use crate::notify;
use crate::paths::Dirs;
use crate::pending::Pending;
use crate::report::{Inputs, Report, read_manifest};
use crate::resolved::Watched;
use crate::watch::Watch;

/// Runs one check. A busy lock exits at once, in silence; an unreachable remote is
/// logged and is not an error.
pub fn check() -> anyhow::Result<()> {
    let dirs = Dirs::from_env()?;
    let Some(_lock) = Lock::try_take(&dirs)? else {
        return Ok(());
    };
    match run(&dirs) {
        Ok(text) => {
            checklog::write(&dirs, &text);
            Ok(())
        }
        Err(e) => {
            checklog::write(&dirs, &format!("erro: {e:#}"));
            Err(e)
        }
    }
}

/// When the last check reached the remote.
pub fn last_success(dirs: &Dirs) -> Option<SystemTime> {
    std::fs::metadata(dirs.last_check_file())
        .and_then(|m| m.modified())
        .ok()
}

fn run(dirs: &Dirs) -> anyhow::Result<String> {
    let inputs = Inputs::load()?;
    let repo = inputs.machine.repo.clone();
    let git_ref = inputs.machine.git_ref.clone();
    let remote = match git::resolve(&repo, &git_ref) {
        Ok(sha) => sha,
        Err(e) => return Ok(format!("remoto inacessivel, nada mudou: {e:#}")),
    };
    mark_success(dirs);
    let watch = Watch::load(&dirs.watch_file());
    if watch.is_some_and(|w| w.unchanged(&remote)) {
        return Ok(format!("sem novidade em {remote}"));
    }
    let snapshot = match Snapshot::cached(&dirs.snapshot_cache(), &repo, &git_ref, &remote) {
        Ok(snapshot) => snapshot,
        Err(e) => return Ok(format!("snapshot de {remote} falhou, nada mudou: {e:#}")),
    };
    let now = checklog::utc_now();
    let found = match read_manifest(&snapshot) {
        Ok(manifest) => {
            let report = Report::plan(inputs, remote.clone(), snapshot, manifest, false)?;
            let found = Pending::changes(&report, now);
            save_watch(dirs, &remote, &report.watched)?;
            report.snapshot.close()?;
            found
        }
        Err(e) => {
            snapshot.close()?;
            save_watch(dirs, &remote, &[])?;
            Some(Pending::invalid(&remote, &format!("{e:#}"), now))
        }
    };
    publish(dirs, &remote, found)
}

fn save_watch(dirs: &Dirs, remote: &str, sources: &[Watched]) -> anyhow::Result<()> {
    let watch = Watch {
        commit: remote.to_string(),
        sources: sources.to_vec(),
    };
    watch.save(&dirs.watch_file())
}

/// Writes or clears `pending.json`; notifies when the news is new.
fn publish(dirs: &Dirs, remote: &str, found: Option<Pending>) -> anyhow::Result<String> {
    let file = dirs.pending_file();
    let Some(pending) = found else {
        Pending::clear(&file)?;
        return Ok(format!("nada aplicavel em {remote}"));
    };
    let known = Pending::load(&file).is_some_and(|old| old.same_news(&pending));
    pending.save(&file)?;
    let summary = pending.line();
    if known {
        return Ok(format!("{summary} (ja avisado)"));
    }
    match notify::for_this_machine().notify("agsync", &pending.message()) {
        Ok(()) => Ok(format!("{summary} (notificado)")),
        Err(e) => Ok(format!("{summary} (notificacao nao mostrada: {e:#})")),
    }
}

fn mark_success(dirs: &Dirs) {
    let stamp = dirs.last_check_file();
    if let Err(e) = touch(&stamp) {
        let text = format!("aviso: gravar {} falhou: {e:#}", stamp.display());
        checklog::write(dirs, &text);
    }
}

fn touch(path: &Path) -> anyhow::Result<()> {
    crate::atomic::write_atomic(path, checklog::utc_now().as_bytes())
}
