//! Linux desktop notification through `notify-send`, only inside a graphical session.

use std::process::{Command, Stdio};

use anyhow::Context;

use super::Notifier;

pub struct NotifySend;

impl Notifier for NotifySend {
    fn notify(&self, title: &str, text: &str) -> anyhow::Result<()> {
        if !graphical(&var("DISPLAY"), &var("WAYLAND_DISPLAY"), is_wsl()) {
            anyhow::bail!("sem sessao grafica (DISPLAY/WAYLAND_DISPLAY ausentes ou WSL)");
        }
        let exe = crate::which::find("notify-send").context("notify-send nao esta no PATH")?;
        let status = Command::new(exe)
            .args(["--app-name=agsync", title, text])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("executar notify-send")?;
        if !status.success() {
            anyhow::bail!("notify-send saiu com {status}");
        }
        Ok(())
    }
}

fn var(name: &str) -> String {
    std::env::var(name).unwrap_or_default()
}

fn is_wsl() -> bool {
    let release = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
    std::env::var_os("WSL_DISTRO_NAME").is_some() || release.to_lowercase().contains("microsoft")
}

fn graphical(display: &str, wayland: &str, wsl: bool) -> bool {
    let session = !display.is_empty() || !wayland.is_empty();
    session && !wsl
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_a_display_outside_wsl() {
        assert!(graphical(":0", "", false));
        assert!(graphical("", "wayland-0", false));
        assert!(!graphical("", "", false));
        assert!(!graphical(":0", "", true));
    }
}
