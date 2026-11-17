//! Temporary directory removed on drop, built on std only.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A fresh directory under the system temp dir, deleted when dropped.
#[derive(Debug)]
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// Creates `<temp>/<prefix>-<pid>-<nanos>-<n>`.
    pub fn new(prefix: &str) -> std::io::Result<TempDir> {
        TempDir::new_in(&std::env::temp_dir(), prefix)
    }

    /// Creates `<parent>/<prefix>-<pid>-<nanos>-<n>`, `parent` included.
    pub fn new_in(parent: &Path, prefix: &str) -> std::io::Result<TempDir> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let name = format!("{prefix}-{}-{nanos}-{n}", std::process::id());
        let path = parent.join(name);
        std::fs::create_dir_all(&path)?;
        Ok(TempDir { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Deletes the directory now and reports failure instead of ignoring it.
    pub fn close(self) -> std::io::Result<()> {
        let path = self.path.clone();
        std::mem::forget(self);
        remove(&path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = remove(&self.path);
    }
}

fn remove(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removed_on_drop_and_close() {
        let a = TempDir::new("agsync-test").unwrap();
        let b = TempDir::new("agsync-test").unwrap();
        assert_ne!(a.path(), b.path());
        std::fs::write(a.path().join("f"), "x").unwrap();
        let (pa, pb) = (a.path().to_path_buf(), b.path().to_path_buf());
        drop(a);
        b.close().unwrap();
        assert!(!pa.exists());
        assert!(!pb.exists());
    }
}
