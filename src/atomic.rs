//! Atomic file write: temp file in the same folder, then rename.

use std::path::Path;

use anyhow::Context;

/// Writes `bytes` to `path` so readers never see a partial file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path.parent().context("caminho sem pasta pai")?;
    std::fs::create_dir_all(parent).with_context(|| format!("criar pasta {}", parent.display()))?;
    let file_name = path.file_name().context("caminho sem nome")?;
    let tmp = parent.join(format!(
        ".{}.tmp-{}",
        file_name.to_string_lossy(),
        std::process::id()
    ));
    std::fs::write(&tmp, bytes).with_context(|| format!("gravar {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("renomear para {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_and_replaces() {
        let dir = crate::tmp::TempDir::new("agsync-test").unwrap();
        let file = dir.path().join("sub").join("f.json");
        write_atomic(&file, b"one").unwrap();
        write_atomic(&file, b"two").unwrap();
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "two");
        let entries = std::fs::read_dir(file.parent().unwrap()).unwrap().count();
        assert_eq!(entries, 1);
    }
}
