//! `state.json`: what agsync wrote, so it only ever touches its own entries.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};

/// How a skill folder was placed in the agent's skills dir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkMode {
    Symlink,
    Junction,
    Copy,
}

/// One entry written by agsync. `hash` never depends on secret values.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tracked {
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<LinkMode>,
    /// Commit of the git source, for items built or copied from one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha: Option<String>,
}

impl Tracked {
    /// Identity compared with [`crate::plan::Want::key`].
    pub fn key(&self) -> String {
        state_key(&self.hash, self.sha.as_deref())
    }
}

/// `hash`, plus `@sha` for items from a git source.
pub fn state_key(hash: &str, sha: Option<&str>) -> String {
    match sha {
        Some(sha) => format!("{hash}@{sha}"),
        None => hash.to_string(),
    }
}

/// Entries written for one agent.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentState {
    #[serde(default)]
    pub mcps: BTreeMap<String, Tracked>,
    #[serde(default)]
    pub skills: BTreeMap<String, Tracked>,
}

/// Whole state file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub commit: Option<String>,
    #[serde(default)]
    pub agents: BTreeMap<String, AgentState>,
}

impl State {
    /// Missing file means empty state.
    pub fn load(path: &Path) -> anyhow::Result<State> {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(State::default()),
            Err(e) => return Err(e).with_context(|| format!("ler {}", path.display())),
        };
        serde_json::from_str(&text).with_context(|| format!("{} invalido", path.display()))
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        crate::atomic::write_atomic(path, text.as_bytes())
    }

    pub fn agent(&self, name: &str) -> AgentState {
        self.agents.get(name).cloned().unwrap_or_default()
    }

    pub fn agent_mut(&mut self, name: &str) -> &mut AgentState {
        self.agents.entry(name.to_string()).or_default()
    }
}

/// Name to key view, as the planner wants it.
pub fn keys(entries: &BTreeMap<String, Tracked>) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(name, t)| (name.clone(), t.key()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_file_is_empty_and_round_trips() {
        let dir = crate::tmp::TempDir::new("agsync-test").unwrap();
        let file = dir.path().join("state.json");
        assert_eq!(State::load(&file).unwrap(), State::default());

        let mut state = State {
            commit: Some("abc".into()),
            ..State::default()
        };
        let claude = state.agent_mut("claude");
        claude.mcps.insert(
            "echo".into(),
            Tracked {
                hash: "h1".into(),
                mode: None,
                sha: None,
            },
        );
        claude.skills.insert(
            "hello".into(),
            Tracked {
                hash: "h2".into(),
                mode: Some(LinkMode::Junction),
                sha: Some("abc".into()),
            },
        );
        state.save(&file).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("\"junction\""));
        assert_eq!(State::load(&file).unwrap(), state);
        assert_eq!(keys(&state.agent("claude").mcps)["echo"], "h1");
        assert_eq!(keys(&state.agent("claude").skills)["hello"], "h2@abc");
    }
}
