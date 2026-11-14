//! `~/.apm/`: moved whole into the backup, never deleted.

use std::path::{Path, PathBuf};

use anyhow::Context;

use super::Leftover;

struct ApmDir(PathBuf);

impl Leftover for ApmDir {
    fn category(&self) -> &'static str {
        "APM"
    }

    fn describe(&self) -> String {
        format!("{} (movida para o backup)", self.0.display())
    }

    fn remove(&self, backup: &Path) -> anyhow::Result<()> {
        let dest = backup.join("apm");
        if std::fs::rename(&self.0, &dest).is_ok() {
            return Ok(());
        }
        crate::agent::link::copy_dir(&self.0, &dest)?;
        std::fs::remove_dir_all(&self.0).with_context(|| format!("apagar {}", self.0.display()))
    }
}

/// `<home>/.apm`, when it exists.
pub fn leftover(home: &Path) -> Option<Box<dyn Leftover>> {
    let dir = home.join(".apm");
    dir.is_dir()
        .then(|| Box::new(ApmDir(dir)) as Box<dyn Leftover>)
}
