//! PowerShell: Windows PowerShell 5.1 (`powershell`) and PowerShell 7 (`pwsh`) keep
//! separate profiles. agsync writes `$PROFILE.CurrentUserAllHosts` of each.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::Shell;

/// Documents folder to use instead of asking PowerShell (tests).
pub const DOCUMENTS_VAR: &str = "AGSYNC_DOCUMENTS_DIR";

pub struct PowerShell {
    name: &'static str,
    exe: &'static str,
    /// Folder under Documents on Windows: `WindowsPowerShell` or `PowerShell`.
    folder: &'static str,
}

impl PowerShell {
    pub fn windows() -> PowerShell {
        PowerShell {
            name: "Windows PowerShell 5.1",
            exe: "powershell",
            folder: "WindowsPowerShell",
        }
    }

    pub fn core() -> PowerShell {
        PowerShell {
            name: "PowerShell 7",
            exe: "pwsh",
            folder: "PowerShell",
        }
    }

    /// Asks the shell itself, so a Documents folder moved to OneDrive is honored.
    fn asked_profile(&self) -> Option<PathBuf> {
        let exe = crate::which::find(self.exe)?;
        let out = Command::new(exe)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "$PROFILE.CurrentUserAllHosts",
            ])
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .ok()?;
        let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
        (out.status.success() && !path.is_empty()).then(|| PathBuf::from(path))
    }
}

impl Shell for PowerShell {
    fn name(&self) -> &'static str {
        self.name
    }

    fn present(&self) -> bool {
        crate::which::find(self.exe).is_some()
    }

    fn profile(&self) -> Option<PathBuf> {
        match std::env::var_os(DOCUMENTS_VAR) {
            Some(docs) => Some(PathBuf::from(docs).join(self.folder).join("profile.ps1")),
            None => self.asked_profile(),
        }
    }

    fn body(&self, exe: &Path) -> String {
        let exe = ps_quote(&exe.to_string_lossy());
        format!("if (Test-Path -LiteralPath {exe}) {{ & {exe} hook }}")
    }
}

fn ps_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_quotes_the_path() {
        let body = PowerShell::core().body(Path::new(r"C:\Users\o'neil\agsync.exe"));
        assert_eq!(
            body,
            r"if (Test-Path -LiteralPath 'C:\Users\o''neil\agsync.exe') { & 'C:\Users\o''neil\agsync.exe' hook }"
        );
    }
}
