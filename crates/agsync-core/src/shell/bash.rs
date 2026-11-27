//! bash: `~/.bashrc`, read by interactive shells.

use std::path::{Path, PathBuf};

use super::{Shell, sh_quote};

pub struct Bash;

impl Shell for Bash {
    fn name(&self) -> &'static str {
        "bash"
    }

    fn present(&self) -> bool {
        crate::which::find("bash").is_some()
    }

    fn profile(&self) -> Option<PathBuf> {
        Some(crate::paths::home().ok()?.join(".bashrc"))
    }

    fn body(&self, exe: &Path) -> String {
        posix_body(exe)
    }
}

/// Same line for bash and zsh.
pub fn posix_body(exe: &Path) -> String {
    let exe = sh_quote(&exe.to_string_lossy());
    format!("if [ -x {exe} ]; then {exe} hook; fi")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_runs_hook_when_the_binary_exists() {
        assert_eq!(
            Bash.body(Path::new("/home/ana/bin/agsync")),
            "if [ -x '/home/ana/bin/agsync' ]; then '/home/ana/bin/agsync' hook; fi"
        );
    }
}
