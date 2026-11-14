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

/// When set to `agsync-test-<anything>`, the OS scheduler is used under that name
/// instead of [`NAME`], so tests never create the real schedule. On Linux the systemd
/// units then live in the runtime dir ([`systemd::Systemd::runtime`]).
pub const TEST_NAME_VAR: &str = "AGSYNC_TEST_SCHEDULE_NAME";

/// The file scheduler when [`TEST_FILE_VAR`] is set, else the one of this OS.
pub fn for_this_machine() -> Box<dyn Scheduler> {
    match std::env::var_os(TEST_FILE_VAR) {
        Some(path) => Box::new(file::FileScheduler(PathBuf::from(path))),
        None => system(),
    }
}

/// The name from [`TEST_NAME_VAR`], when it has the test prefix.
fn test_name() -> Option<String> {
    let name = std::env::var(TEST_NAME_VAR).ok()?;
    name.starts_with("agsync-test-").then_some(name)
}

#[cfg(windows)]
fn system() -> Box<dyn Scheduler> {
    let name = test_name().unwrap_or_else(|| NAME.to_string());
    Box::new(task::TaskScheduler::new(&name))
}

#[cfg(not(windows))]
fn system() -> Box<dyn Scheduler> {
    use cron::Cron;
    use systemd::Systemd;
    let linux = match test_name() {
        Some(name) => linux::Linux::detect(Systemd::runtime(&name), Cron::new(&name)),
        None => linux::Linux::detect(Systemd::for_user(NAME), Cron::new(NAME)),
    };
    Box::new(linux)
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
