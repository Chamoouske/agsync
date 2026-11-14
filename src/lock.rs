//! `<data>/agsync.lock`: one run at a time may write `state.json` and `pending.json`.

use std::fs::{File, OpenOptions, TryLockError};

use anyhow::Context;

use crate::paths::Dirs;

/// Held lock, released when dropped (also when the process dies).
#[derive(Debug)]
pub struct Lock {
    _file: File,
}

impl Lock {
    /// Takes the lock now, or returns `None` when another run holds it.
    pub fn try_take(dirs: &Dirs) -> anyhow::Result<Option<Lock>> {
        let file = open(dirs)?;
        match file.try_lock() {
            Ok(()) => Ok(Some(Lock { _file: file })),
            Err(TryLockError::WouldBlock) => Ok(None),
            Err(TryLockError::Error(e)) => Err(e).context("travar agsync.lock"),
        }
    }

    /// Waits for the lock; says so on stderr when another run holds it.
    pub fn wait(dirs: &Dirs) -> anyhow::Result<Lock> {
        if let Some(lock) = Lock::try_take(dirs)? {
            return Ok(lock);
        }
        eprintln!("outro agsync (check ou apply) esta rodando; esperando ele terminar...");
        let file = open(dirs)?;
        file.lock().context("esperar agsync.lock")?;
        Ok(Lock { _file: file })
    }
}

fn open(dirs: &Dirs) -> anyhow::Result<File> {
    let path = dirs.lock_file();
    std::fs::create_dir_all(&dirs.data)
        .with_context(|| format!("criar {}", dirs.data.display()))?;
    OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .with_context(|| format!("abrir {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_taker_is_refused_until_release() {
        let tmp = crate::tmp::TempDir::new("agsync-test").unwrap();
        let dirs = Dirs {
            config: tmp.path().join("config"),
            data: tmp.path().join("data"),
        };
        let first = Lock::try_take(&dirs).unwrap().expect("free lock");
        assert!(Lock::try_take(&dirs).unwrap().is_none());
        drop(first);
        let again = Lock::try_take(&dirs).unwrap();
        assert!(again.is_some());
        drop(again);
        assert!(Lock::wait(&dirs).is_ok());
    }
}
