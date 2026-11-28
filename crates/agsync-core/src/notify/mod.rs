//! Desktop notifications: one small interface, one implementation per OS. The host
//! ([`crate::host`]) decides whether the OS one is used at all.

use std::path::PathBuf;

mod file;
#[cfg(not(windows))]
pub mod notify_send;
#[cfg(windows)]
mod toast;

/// Shows one short message to the user.
pub trait Notifier {
    fn notify(&self, title: &str, text: &str) -> anyhow::Result<()>;
}

/// When set, notifications are appended to this file instead of shown (tests).
pub const TEST_LOG_VAR: &str = "AGSYNC_TEST_NOTIFY_LOG";

/// The file notifier when [`TEST_LOG_VAR`] is set, else the one of this host.
pub fn for_this_machine() -> Box<dyn Notifier> {
    match std::env::var_os(TEST_LOG_VAR) {
        Some(path) => Box::new(file::FileNotifier(PathBuf::from(path))),
        None => crate::host::current().notifier(),
    }
}

/// Shows nothing: the news stays in the terminal. The text says why, for `check.log`.
pub struct TerminalOnly(pub &'static str);

impl Notifier for TerminalOnly {
    fn notify(&self, _title: &str, _text: &str) -> anyhow::Result<()> {
        anyhow::bail!("{}", self.0)
    }
}

/// The desktop notifier of this OS.
#[cfg(windows)]
pub fn system() -> Box<dyn Notifier> {
    Box::new(toast::Toast)
}

/// The desktop notifier of this OS.
#[cfg(not(windows))]
pub fn system() -> Box<dyn Notifier> {
    Box::new(notify_send::NotifySend)
}
