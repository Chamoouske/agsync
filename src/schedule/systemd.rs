//! Linux: `agsync-check.service` + `agsync-check.timer` in the systemd user dir.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::Context;

use super::{NAME, Scheduler, Status};

pub struct Systemd {
    /// `~/.config/systemd/user`, or under `XDG_CONFIG_HOME`.
    pub dir: PathBuf,
}

impl Systemd {
    pub fn for_user() -> anyhow::Result<Systemd> {
        let config = match std::env::var_os("XDG_CONFIG_HOME") {
            Some(dir) => PathBuf::from(dir),
            None => crate::paths::home()?.join(".config"),
        };
        Ok(Systemd {
            dir: config.join("systemd").join("user"),
        })
    }

    /// True when `systemctl --user` reaches a user manager.
    pub fn works() -> bool {
        systemctl(&["show-environment"]).is_ok()
    }

    fn service(&self) -> PathBuf {
        self.dir.join(format!("{NAME}.service"))
    }

    fn timer(&self) -> PathBuf {
        self.dir.join(format!("{NAME}.timer"))
    }
}

impl Scheduler for Systemd {
    fn install(&self, exe: &Path, minutes: u32) -> anyhow::Result<Vec<String>> {
        let path = std::env::var("PATH").unwrap_or_default();
        crate::atomic::write_atomic(&self.service(), service_text(exe, &path).as_bytes())?;
        crate::atomic::write_atomic(&self.timer(), timer_text(minutes).as_bytes())?;
        systemctl(&["daemon-reload"])?;
        systemctl(&["enable", "--now", &format!("{NAME}.timer")])?;
        Ok(vec![frozen_path_note("na unit", &self.service())])
    }

    fn remove(&self) -> anyhow::Result<bool> {
        if !self.service().exists() && !self.timer().exists() {
            return Ok(false);
        }
        let _ = systemctl(&["disable", "--now", &format!("{NAME}.timer")]);
        for file in [self.timer(), self.service()] {
            if file.exists() {
                std::fs::remove_file(&file)
                    .with_context(|| format!("apagar {}", file.display()))?;
            }
        }
        let _ = systemctl(&["daemon-reload"]);
        Ok(true)
    }

    fn status(&self, exe: &Path) -> anyhow::Result<Status> {
        let Ok(text) = std::fs::read_to_string(self.service()) else {
            return Ok(Status::Missing);
        };
        let line = text
            .lines()
            .find(|l| l.starts_with("ExecStart="))
            .unwrap_or_default();
        if line == exec_line(exe) {
            return Ok(Status::Current);
        }
        Ok(Status::Other(line.to_string()))
    }
}

/// Note shown after install: builds and agents need tools from the PATH saved now.
pub fn frozen_path_note(place: &str, file: &Path) -> String {
    format!(
        "o PATH atual ficou gravado {place} ({}): node, uv, java e docker que os builds usam precisam estar nele; depois de mudar o PATH, rode `agsync schedule install` de novo",
        file.display()
    )
}

pub fn service_text(exe: &Path, path: &str) -> String {
    format!(
        "[Unit]\nDescription=agsync: le o repo de config e avisa mudancas pendentes\n\n[Service]\nType=oneshot\n{}\nEnvironment={}\nTimeoutStartSec=10min\n",
        exec_line(exe),
        quote(&format!("PATH={path}"))
    )
}

pub fn timer_text(minutes: u32) -> String {
    format!(
        "[Unit]\nDescription=agsync check a cada {minutes} min\n\n[Timer]\nOnBootSec=2min\nOnUnitActiveSec={minutes}min\n\n[Install]\nWantedBy=timers.target\n"
    )
}

fn exec_line(exe: &Path) -> String {
    format!("ExecStart={} check", quote(&exe.to_string_lossy()))
}

/// Double-quoted systemd word: `\` and `"` escaped, `%` doubled (no specifiers).
fn quote(text: &str) -> String {
    let escaped = text
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('%', "%%");
    format!("\"{escaped}\"")
}

fn systemctl(args: &[&str]) -> anyhow::Result<()> {
    let out = Command::new("systemctl")
        .arg("--user")
        .args(args)
        .stdin(Stdio::null())
        .output()
        .context("executar systemctl --user")?;
    if !out.status.success() {
        anyhow::bail!(
            "systemctl --user {} falhou: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_runs_check_with_frozen_path() {
        let text = service_text(Path::new("/home/ana/bin/agsync"), "/usr/bin:/a%b");
        assert!(text.contains("\nType=oneshot\n"));
        assert!(text.contains("\nExecStart=\"/home/ana/bin/agsync\" check\n"));
        assert!(text.contains("\nEnvironment=\"PATH=/usr/bin:/a%%b\"\n"));
        assert!(text.contains("\nTimeoutStartSec=10min\n"));
    }

    #[test]
    fn timer_repeats_after_boot_and_each_run() {
        let text = timer_text(15);
        assert!(text.contains("\nOnBootSec=2min\n"));
        assert!(text.contains("\nOnUnitActiveSec=15min\n"));
        assert!(text.contains("\nWantedBy=timers.target\n"));
    }

    #[test]
    fn status_compares_exec_start() {
        let tmp = crate::tmp::TempDir::new("agsync-test").unwrap();
        let systemd = Systemd {
            dir: tmp.path().to_path_buf(),
        };
        let exe = Path::new("/opt/my \"x\"/agsync");
        assert_eq!(systemd.status(exe).unwrap(), Status::Missing);
        std::fs::write(systemd.service(), service_text(exe, "/bin")).unwrap();
        assert_eq!(systemd.status(exe).unwrap(), Status::Current);
        let other = systemd.status(Path::new("/old/agsync")).unwrap();
        assert!(matches!(other, Status::Other(line) if line.contains("my \\\"x\\\"")));
    }
}
