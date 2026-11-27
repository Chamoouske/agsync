//! zsh: `$ZDOTDIR/.zshrc`, else `~/.zshrc`.

use std::path::{Path, PathBuf};

use super::Shell;
use super::bash::posix_body;

pub struct Zsh;

impl Shell for Zsh {
    fn name(&self) -> &'static str {
        "zsh"
    }

    fn present(&self) -> bool {
        crate::which::find("zsh").is_some()
    }

    fn profile(&self) -> Option<PathBuf> {
        let dir = match std::env::var_os("ZDOTDIR") {
            Some(dir) => PathBuf::from(dir),
            None => crate::paths::home().ok()?,
        };
        Some(dir.join(".zshrc"))
    }

    fn body(&self, exe: &Path) -> String {
        posix_body(exe)
    }
}
