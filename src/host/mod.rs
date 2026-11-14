//! Where agsync runs: a plain OS, or Linux inside WSL, which sees the Windows drives and
//! executables but has a home and agents of its own. One small interface, picked once.

use std::sync::OnceLock;

use crate::notify::Notifier;

mod native;
#[cfg(any(not(windows), test))]
pub mod wsl;

/// What changes with the place agsync runs in.
pub trait Host: Send + Sync {
    /// How `agsync check` tells the user about news.
    fn notifier(&self) -> Box<dyn Notifier>;
}

/// The host of this process, detected on first use.
pub fn current() -> &'static dyn Host {
    static HOST: OnceLock<Box<dyn Host>> = OnceLock::new();
    HOST.get_or_init(detect).as_ref()
}

#[cfg(windows)]
fn detect() -> Box<dyn Host> {
    Box::new(native::Native)
}

#[cfg(not(windows))]
fn detect() -> Box<dyn Host> {
    match wsl::Wsl::detect() {
        Some(wsl) => Box::new(wsl),
        None => Box::new(native::Native),
    }
}
