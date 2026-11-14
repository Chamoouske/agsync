//! Scheduler that keeps `<exe><TAB><minutes>` in a file, so tests never touch the OS.

use std::path::{Path, PathBuf};

use super::{Scheduler, Status};

pub struct FileScheduler(pub PathBuf);

impl Scheduler for FileScheduler {
    fn install(&self, exe: &Path, minutes: u32) -> anyhow::Result<Vec<String>> {
        let line = format!("{}\t{minutes}\n", exe.display());
        crate::atomic::write_atomic(&self.0, line.as_bytes())?;
        Ok(Vec::new())
    }

    fn remove(&self) -> anyhow::Result<bool> {
        match std::fs::remove_file(&self.0) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    fn status(&self, exe: &Path) -> anyhow::Result<Status> {
        let Ok(text) = std::fs::read_to_string(&self.0) else {
            return Ok(Status::Missing);
        };
        let found = text.split('\t').next().unwrap_or_default();
        if Path::new(found) == exe {
            return Ok(Status::Current);
        }
        Ok(Status::Other(found.to_string()))
    }
}
