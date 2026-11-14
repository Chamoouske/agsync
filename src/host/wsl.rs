//! Linux inside WSL.
//!
//! - Executables under the Windows drives (`/mnt/<letter>/`, or the `root` of
//!   `[automount]` in `/etc/wsl.conf`) are Windows programs reached by interop. Running
//!   them would make this agsync write the Windows configs, so they are ignored.
//! - No desktop notification, even with WSLg's `DISPLAY`: news only shows in the
//!   terminal (`agsync hook`, `status`).

use std::path::{Component, Path, PathBuf};

use super::Host;
use crate::notify::{Notifier, TerminalOnly};

pub struct Wsl {
    /// `WSL_DISTRO_NAME`, when set.
    pub distro: Option<String>,
    /// Where the Windows drives are mounted, `/mnt/` by default.
    pub mount_root: PathBuf,
}

impl Wsl {
    /// Some when `/proc/sys/kernel/osrelease` names Microsoft or `WSL_DISTRO_NAME` is set.
    pub fn detect() -> Option<Wsl> {
        let release = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
        let distro = std::env::var("WSL_DISTRO_NAME").ok();
        if !is_wsl(&release, distro.is_some()) {
            return None;
        }
        let conf = std::fs::read_to_string("/etc/wsl.conf").unwrap_or_default();
        Some(Wsl {
            distro,
            mount_root: mount_root(&conf),
        })
    }
}

impl Host for Wsl {
    fn foreign(&self, exe: &Path) -> bool {
        let Ok(rest) = exe.strip_prefix(&self.mount_root) else {
            return false;
        };
        let Some(Component::Normal(drive)) = rest.components().next() else {
            return false;
        };
        let drive = drive.to_string_lossy();
        drive.len() == 1 && drive.chars().all(|c| c.is_ascii_alphabetic())
    }

    fn notifier(&self) -> Box<dyn Notifier> {
        Box::new(TerminalOnly(
            "WSL: so aviso no terminal (`agsync hook`, `agsync status`)",
        ))
    }

    fn doctor_note(&self) -> Option<String> {
        let distro = self.distro.as_deref().unwrap_or("distro sem nome");
        let home = crate::paths::home()
            .map(|h| h.display().to_string())
            .unwrap_or_else(|_| "$HOME".to_string());
        Some(format!(
            "WSL ({distro}): as configs dos agentes sao as do home Linux ({home}), nao as do Windows; o Windows precisa de instalacao propria do agsync",
        ))
    }
}

/// True for a WSL kernel release or when the distro name variable is set.
pub fn is_wsl(osrelease: &str, distro_var: bool) -> bool {
    distro_var || osrelease.to_lowercase().contains("microsoft")
}

/// `root` of the `[automount]` section of a `wsl.conf`, else `/mnt/`.
pub fn mount_root(conf: &str) -> PathBuf {
    let mut section = String::new();
    for line in conf.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.trim().to_lowercase();
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let is_root = section == "automount" && key.trim().eq_ignore_ascii_case("root");
        let value = value.trim().trim_matches(['"', '\'']);
        if is_root && value.starts_with('/') {
            return PathBuf::from(value);
        }
    }
    PathBuf::from("/mnt/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wsl(root: &str) -> Wsl {
        Wsl {
            distro: Some("Ubuntu".into()),
            mount_root: PathBuf::from(root),
        }
    }

    #[test]
    fn detects_wsl_from_release_or_variable() {
        assert!(is_wsl("6.6.87.2-microsoft-standard-WSL2", false));
        assert!(is_wsl("4.4.0-19041-Microsoft", false));
        assert!(is_wsl("6.8.0-45-generic", true));
        assert!(!is_wsl("6.8.0-45-generic", false));
    }

    #[test]
    fn never_notifies() {
        let error = wsl("/mnt/").notifier().notify("agsync", "x").unwrap_err();
        assert!(format!("{error}").contains("WSL"), "{error}");
    }

    #[test]
    fn doctor_says_wsl_uses_the_linux_home() {
        let note = wsl("/mnt/").doctor_note().unwrap();
        assert!(note.starts_with("WSL (Ubuntu): "), "{note}");
        assert!(note.contains("home Linux"), "{note}");
        assert!(note.contains("instalacao propria do agsync"), "{note}");
    }

    #[test]
    fn reads_the_automount_root() {
        assert_eq!(mount_root(""), PathBuf::from("/mnt/"));
        let conf = "[boot]\nsystemd=true\n[automount]\nenabled = true\nroot = /win/\n";
        assert_eq!(mount_root(conf), PathBuf::from("/win/"));
        assert_eq!(mount_root("[automount]\nroot=\"/\"\n"), PathBuf::from("/"));
        assert_eq!(
            mount_root("[network]\nroot = /x/\n"),
            PathBuf::from("/mnt/")
        );
    }

    #[test]
    fn windows_drives_are_foreign() {
        let host = wsl("/mnt/");
        assert!(host.foreign(Path::new("/mnt/c/Users/ana/AppData/Roaming/npm/claude")));
        assert!(host.foreign(Path::new("/mnt/d/tools/git")));
        assert!(!host.foreign(Path::new("/mnt/data/bin/claude")));
        assert!(!host.foreign(Path::new("/usr/bin/git")));
        assert!(!host.foreign(Path::new("/home/ana/.local/bin/claude")));
        let root = wsl("/");
        assert!(root.foreign(Path::new("/c/Windows/System32/cmd.exe")));
        assert!(!root.foreign(Path::new("/usr/bin/git")));
    }
}
