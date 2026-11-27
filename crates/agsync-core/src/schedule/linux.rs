//! Linux: the systemd user timer when `systemctl --user` works, else a crontab line.

use std::path::Path;

use super::cron::Cron;
use super::systemd::Systemd;
use super::{Scheduler, Status};

pub struct Linux {
    systemd: Option<Systemd>,
    systemd_works: bool,
    cron: Option<Cron>,
}

impl Linux {
    /// Picks what works on this machine; `systemd` and `cron` name the same schedule.
    pub fn detect(systemd: anyhow::Result<Systemd>, cron: anyhow::Result<Cron>) -> Linux {
        Linux {
            systemd: systemd.ok(),
            systemd_works: Systemd::works(),
            cron: cron.ok().filter(|_| Cron::available()),
        }
    }
}

impl Scheduler for Linux {
    fn install(&self, exe: &Path, minutes: u32) -> anyhow::Result<Vec<String>> {
        if let (Some(systemd), true) = (&self.systemd, self.systemd_works) {
            return systemd.install(exe, minutes);
        }
        if let Some(cron) = &self.cron {
            return cron.install(exe, minutes);
        }
        anyhow::bail!(
            "sem `systemctl --user` funcional nem `crontab`: o `agsync hook` dispara o check quando o shell abre"
        )
    }

    fn remove(&self) -> anyhow::Result<bool> {
        let mut removed = false;
        if let Some(systemd) = &self.systemd {
            removed |= systemd.remove()?;
        }
        if let Some(cron) = &self.cron {
            removed |= cron.remove()?;
        }
        Ok(removed)
    }

    fn status(&self, exe: &Path) -> anyhow::Result<Status> {
        if let Some(systemd) = &self.systemd {
            let found = systemd.status(exe)?;
            if found != Status::Missing {
                return Ok(found);
            }
        }
        if let Some(cron) = &self.cron {
            return cron.status(exe);
        }
        Ok(Status::Missing)
    }
}
