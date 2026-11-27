//! Child processes that outlive the parent and never show a window.

use std::process::Command;

/// Windows: a hidden console of its own (its `git` children share it, so no window
/// flashes) in a new process group, out of reach of the terminal's Ctrl+C.
#[cfg(windows)]
pub fn detach(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::System::Threading::{CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW};
    cmd.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
}

/// Unix: a process group of its own, so the terminal's signals do not reach it.
#[cfg(not(windows))]
pub fn detach(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    cmd.process_group(0);
}
