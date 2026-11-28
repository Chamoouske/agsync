//! fish: a file of its own, `~/.config/fish/conf.d/agsync.fish`.

use std::path::{Path, PathBuf};

use super::Shell;

pub struct Fish;

impl Shell for Fish {
    fn name(&self) -> &'static str {
        "fish"
    }

    fn present(&self) -> bool {
        crate::which::find("fish").is_some()
    }

    fn profile(&self) -> Option<PathBuf> {
        let config = match std::env::var_os("XDG_CONFIG_HOME") {
            Some(dir) => PathBuf::from(dir),
            None => crate::paths::home().ok()?.join(".config"),
        };
        Some(config.join("fish").join("conf.d").join("agsync.fish"))
    }

    fn body(&self, exe: &Path) -> String {
        let exe = fish_quote(&exe.to_string_lossy());
        format!("if status is-interactive; and test -x {exe}\n    {exe} hook\nend")
    }
}

fn fish_quote(text: &str) -> String {
    format!("'{}'", text.replace('\\', "\\\\").replace('\'', "\\'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_runs_hook_in_interactive_shells() {
        assert_eq!(
            Fish.body(Path::new("/opt/it's/agsync")),
            "if status is-interactive; and test -x '/opt/it\\'s/agsync'\n    '/opt/it\\'s/agsync' hook\nend"
        );
    }
}
