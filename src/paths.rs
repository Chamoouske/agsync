//! Local config and data directories.

use std::path::PathBuf;

use anyhow::Context;

/// Where agsync keeps machine config and data.
#[derive(Debug, Clone)]
pub struct Dirs {
    pub config: PathBuf,
    pub data: PathBuf,
}

impl Dirs {
    /// Honors `AGSYNC_CONFIG_DIR` and `AGSYNC_DATA_DIR`, else the OS defaults.
    pub fn from_env() -> anyhow::Result<Dirs> {
        let config = match std::env::var_os("AGSYNC_CONFIG_DIR") {
            Some(dir) => PathBuf::from(dir),
            None => default_config()?,
        };
        let data = match std::env::var_os("AGSYNC_DATA_DIR") {
            Some(dir) => PathBuf::from(dir),
            None => default_data()?,
        };
        Ok(Dirs { config, data })
    }

    pub fn machine_file(&self) -> PathBuf {
        self.config.join("machine.toml")
    }

    pub fn state_file(&self) -> PathBuf {
        self.data.join("state.json")
    }

    pub fn backups(&self) -> PathBuf {
        self.data.join("backups")
    }

    pub fn skill_cache(&self) -> PathBuf {
        self.data.join("skills")
    }

    pub fn sources(&self) -> PathBuf {
        self.data.join("sources")
    }

    pub fn logs(&self) -> PathBuf {
        self.data.join("logs")
    }

    pub fn lock_file(&self) -> PathBuf {
        self.data.join("agsync.lock")
    }

    pub fn pending_file(&self) -> PathBuf {
        self.data.join("pending.json")
    }

    pub fn watch_file(&self) -> PathBuf {
        self.data.join("watch.json")
    }

    pub fn last_check_file(&self) -> PathBuf {
        self.data.join("last-check")
    }
}

/// User home: `USERPROFILE` on Windows, `HOME` elsewhere.
pub fn home() -> anyhow::Result<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    env_dir(var)
}

#[cfg(windows)]
fn default_config() -> anyhow::Result<PathBuf> {
    Ok(env_dir("APPDATA")?.join("agsync"))
}

#[cfg(windows)]
fn default_data() -> anyhow::Result<PathBuf> {
    Ok(env_dir("LOCALAPPDATA")?.join("agsync"))
}

#[cfg(not(windows))]
fn default_config() -> anyhow::Result<PathBuf> {
    Ok(home()?.join(".config").join("agsync"))
}

#[cfg(not(windows))]
fn default_data() -> anyhow::Result<PathBuf> {
    Ok(home()?.join(".local").join("share").join("agsync"))
}

fn env_dir(var: &str) -> anyhow::Result<PathBuf> {
    let value = std::env::var_os(var).with_context(|| format!("variavel {var} nao definida"))?;
    Ok(PathBuf::from(value))
}
