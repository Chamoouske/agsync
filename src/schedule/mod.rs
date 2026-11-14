//! Periodic `agsync check`: one small interface, one implementation per OS.
//!
//! Windows uses a user task in Task Scheduler; Linux a systemd user timer, else a
//! crontab line. [`for_this_machine`] picks one; `install`, `remove` and `status` are
//! the commands, reusable by `init` and `uninstall`.

use std::path::{Path, PathBuf};

use crate::machine::Machine;
use crate::paths::Dirs;

#[cfg(any(not(windows), test))]
pub mod cron;
mod file;
#[cfg(not(windows))]
mod linux;
#[cfg(any(not(windows), test))]
pub mod systemd;
#[cfg(windows)]
pub mod task;

/// Name of the task, unit or crontab marker.
pub const NAME: &str = "agsync-check";

/// When set, the schedule is a line in this file instead of the OS scheduler (tests).
pub const TEST_FILE_VAR: &str = "AGSYNC_TEST_SCHEDULE_FILE";

/// What is registered, compared with the running binary.
#[derive(Debug, PartialEq, Eq)]
pub enum Status {
    Missing,
    /// Runs this very binary.
    Current,
    /// Runs something else, as found in the scheduler.
    Other(String),
}

/// Registers the periodic check with the OS.
pub trait Scheduler {
    /// Creates or replaces the check of `exe` every `minutes`. Returns notes for the user.
    fn install(&self, exe: &Path, minutes: u32) -> anyhow::Result<Vec<String>>;
    /// Deletes the check; false when there was none.
    fn remove(&self) -> anyhow::Result<bool>;
    fn status(&self, exe: &Path) -> anyhow::Result<Status>;
}

/// The file scheduler when [`TEST_FILE_VAR`] is set, else the one of this OS.
pub fn for_this_machine() -> Box<dyn Scheduler> {
    match std::env::var_os(TEST_FILE_VAR) {
        Some(path) => Box::new(file::FileScheduler(PathBuf::from(path))),
        None => system(),
    }
}

#[cfg(windows)]
fn system() -> Box<dyn Scheduler> {
    Box::new(task::TaskScheduler::new(NAME))
}

#[cfg(not(windows))]
fn system() -> Box<dyn Scheduler> {
    Box::new(linux::Linux::detect())
}

/// `agsync schedule install`: every `check_interval_minutes` of `machine.toml` (default 15).
pub fn install() -> anyhow::Result<()> {
    let exe = crate::paths::current_exe()?;
    let minutes = interval()?;
    let notes = for_this_machine().install(&exe, minutes)?;
    println!("agendado: `{} check` a cada {minutes} min", exe.display());
    for note in notes {
        println!("{note}");
    }
    Ok(())
}

/// `agsync schedule remove`.
pub fn remove() -> anyhow::Result<()> {
    if for_this_machine().remove()? {
        println!("agendamento removido");
    } else {
        println!("nenhum agendamento encontrado");
    }
    Ok(())
}

/// `agsync schedule status`. True when the check runs this binary.
pub fn status() -> anyhow::Result<bool> {
    let exe = crate::paths::current_exe()?;
    let found = for_this_machine().status(&exe)?;
    println!("{}", describe(&found, &exe));
    Ok(found == Status::Current)
}

/// One line about `status`, as `schedule status` and `doctor` print it.
pub fn describe(status: &Status, exe: &Path) -> String {
    match status {
        Status::Missing => "agendamento ausente; o `agsync hook` dispara o check quando o shell abre; rode `agsync schedule install`".to_string(),
        Status::Current => format!("agendamento ok: roda `{} check`", exe.display()),
        Status::Other(found) => format!(
            "agendamento aponta para outro binario ({found}); rode `agsync schedule install` para usar {}",
            exe.display()
        ),
    }
}

fn interval() -> anyhow::Result<u32> {
    let dirs = Dirs::from_env()?;
    let file = dirs.machine_file();
    if !file.is_file() {
        return Ok(Machine::new(String::new(), None, Vec::new()).check_interval_minutes);
    }
    let minutes = Machine::load(&file)?.check_interval_minutes;
    if minutes == 0 {
        anyhow::bail!("check_interval_minutes = 0 em {}", file.display());
    }
    Ok(minutes)
}
