//! Timestamped copies of agent config files, taken before agsync changes them.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;

/// Copies `file` to `<dir>/<label>.<millis>`. A missing file needs no backup.
pub fn copy(file: &Path, dir: &Path, label: &str) -> anyhow::Result<()> {
    if !file.is_file() {
        return Ok(());
    }
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    std::fs::create_dir_all(dir)?;
    let dest = dir.join(format!("{label}.{millis}"));
    std::fs::copy(file, &dest).with_context(|| format!("backup de {}", file.display()))?;
    Ok(())
}
