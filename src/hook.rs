//! `agsync hook`: run by the shell at startup. Never touches the network: prints the
//! pending and new version lines and, when no check ran for too long, starts one in the
//! background.

use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, SystemTime};

use anyhow::Context;

use crate::machine::Machine;
use crate::paths::Dirs;
use crate::pending::Pending;

/// Prints the pending and new version lines, if any, and starts a background check when the last one is
/// older than twice the interval (no working scheduler, as in WSL without systemd).
pub fn hook() -> anyhow::Result<()> {
    let dirs = Dirs::from_env()?;
    if let Some(pending) = Pending::load(&dirs.pending_file()) {
        println!("{}", pending.line());
    }
    if let Some(newer) = crate::update::recorded(&dirs) {
        println!("{}", crate::update::line(newer));
    }
    if check_overdue(&dirs) {
        start_check(&dirs)?;
    }
    Ok(())
}

/// The pending and new version lines on stderr, for commands that do not handle
/// `pending.json` themselves.
pub fn warn_pending() {
    let Ok(dirs) = Dirs::from_env() else {
        return;
    };
    if let Some(pending) = Pending::load(&dirs.pending_file()) {
        eprintln!("{}", pending.line());
    }
    if let Some(newer) = crate::update::recorded(&dirs) {
        eprintln!("{}", crate::update::line(newer));
    }
}

/// True when neither a check succeeded within twice the interval nor the hook started
/// one within the interval. Without `machine.toml` there is nothing to check.
fn check_overdue(dirs: &Dirs) -> bool {
    let Ok(machine) = Machine::load(&dirs.machine_file()) else {
        return false;
    };
    let interval = Duration::from_secs(60 * u64::from(machine.check_interval_minutes));
    let checked_long_ago = older_than(crate::check::last_success(dirs), interval * 2);
    let started_long_ago = older_than(modified(&dirs.hook_started_file()), interval);
    checked_long_ago && started_long_ago
}

fn older_than(time: Option<SystemTime>, limit: Duration) -> bool {
    let age = time.and_then(|t| t.elapsed().ok());
    age.is_none_or(|age| age > limit)
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn start_check(dirs: &Dirs) -> anyhow::Result<()> {
    crate::atomic::write_atomic(&dirs.hook_started_file(), b"")?;
    let exe = std::env::current_exe().context("caminho do agsync")?;
    let mut cmd = Command::new(exe);
    cmd.arg("check")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    crate::background::detach(&mut cmd);
    cmd.spawn()
        .context("iniciar `agsync check` em segundo plano")?;
    Ok(())
}
