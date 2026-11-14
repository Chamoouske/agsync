//! Desktop notifications: one small interface, one implementation per OS.

use std::path::PathBuf;

mod file;
#[cfg(not(windows))]
mod notify_send;
#[cfg(windows)]
mod toast;

/// Shows one short message to the user.
pub trait Notifier {
    fn notify(&self, title: &str, text: &str) -> anyhow::Result<()>;
}

/// When set, notifications are appended to this file instead of shown (tests).
pub const TEST_LOG_VAR: &str = "AGSYNC_TEST_NOTIFY_LOG";

/// The file notifier when [`TEST_LOG_VAR`] is set, else the one of this OS.
pub fn for_this_machine() -> Box<dyn Notifier> {
    match std::env::var_os(TEST_LOG_VAR) {
        Some(path) => Box::new(file::FileNotifier(PathBuf::from(path))),
        None => system(),
    }
}

#[cfg(windows)]
fn system() -> Box<dyn Notifier> {
    Box::new(toast::Toast)
}

#[cfg(not(windows))]
fn system() -> Box<dyn Notifier> {
    Box::new(notify_send::NotifySend)
}
