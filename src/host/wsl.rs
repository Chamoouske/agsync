//! Linux inside WSL: no desktop notification, even with WSLg's `DISPLAY`. News only
//! shows in the terminal (`agsync hook`, `status`).

use super::Host;
use crate::notify::{Notifier, TerminalOnly};

pub struct Wsl;

impl Wsl {
    /// Some when `/proc/sys/kernel/osrelease` names Microsoft or `WSL_DISTRO_NAME` is set.
    pub fn detect() -> Option<Wsl> {
        let release = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
        let distro = std::env::var_os("WSL_DISTRO_NAME");
        is_wsl(&release, distro.is_some()).then_some(Wsl)
    }
}

impl Host for Wsl {
    fn notifier(&self) -> Box<dyn Notifier> {
        Box::new(TerminalOnly(
            "WSL: so aviso no terminal (`agsync hook`, `agsync status`)",
        ))
    }
}

/// True for a WSL kernel release or when the distro name variable is set.
pub fn is_wsl(osrelease: &str, distro_var: bool) -> bool {
    distro_var || osrelease.to_lowercase().contains("microsoft")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_wsl_from_release_or_variable() {
        assert!(is_wsl("6.6.87.2-microsoft-standard-WSL2", false));
        assert!(is_wsl("4.4.0-19041-Microsoft", false));
        assert!(is_wsl("6.8.0-45-generic", true));
        assert!(!is_wsl("6.8.0-45-generic", false));
    }

    #[test]
    fn never_notifies() {
        let error = Wsl.notifier().notify("agsync", "x").unwrap_err();
        assert!(format!("{error}").contains("WSL"), "{error}");
    }
}
