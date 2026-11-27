//! `machine.toml`: per-machine settings written by `agsync init`.

use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};

/// Settings for this machine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Machine {
    pub repo: String,
    #[serde(rename = "ref", default = "default_ref")]
    pub git_ref: String,
    #[serde(default)]
    pub profiles: Vec<String>,
    #[serde(default = "default_interval")]
    pub check_interval_minutes: u32,
}

impl Machine {
    pub fn new(repo: String, git_ref: Option<String>, profiles: Vec<String>) -> Machine {
        Machine {
            repo,
            git_ref: git_ref.unwrap_or_else(default_ref),
            profiles,
            check_interval_minutes: default_interval(),
        }
    }

    pub fn load(path: &Path) -> anyhow::Result<Machine> {
        let text = std::fs::read_to_string(path).with_context(|| {
            format!(
                "ler {} (rode `agsync init <url> --profile P` antes)",
                path.display()
            )
        })?;
        toml::from_str(&text).with_context(|| format!("{} invalido", path.display()))
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let text = toml::to_string(self)?;
        crate::atomic::write_atomic(path, text.as_bytes())
    }
}

fn default_ref() -> String {
    "main".to_string()
}

fn default_interval() -> u32 {
    15
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_defaults() {
        let dir = crate::tmp::TempDir::new("agsync-test").unwrap();
        let file = dir.path().join("machine.toml");
        let m = Machine::new("https://x/repo.git".into(), None, vec!["work".into()]);
        m.save(&file).unwrap();
        assert_eq!(Machine::load(&file).unwrap(), m);
        assert_eq!(m.git_ref, "main");
        assert_eq!(m.check_interval_minutes, 15);

        std::fs::write(&file, "repo = \"u\"\n").unwrap();
        let loaded = Machine::load(&file).unwrap();
        assert_eq!(loaded.git_ref, "main");
        assert!(loaded.profiles.is_empty());

        std::fs::write(&file, "repo = \"u\"\nbogus = 1\n").unwrap();
        assert!(Machine::load(&file).is_err());
    }
}
