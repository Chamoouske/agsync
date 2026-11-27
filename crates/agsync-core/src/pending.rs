//! `<data>/pending.json`: what the last `check` found that `apply` would change.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::paths::Dirs;
use crate::plan::Action;
use crate::report::Report;
use crate::resolved::Watched;
use crate::update::Version;

/// News found by `check` for one config commit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pending {
    pub commit: String,
    pub checked_at: String,
    /// Moving sources as planned; a change here is news even at the same commit.
    #[serde(default)]
    pub sources: Vec<Watched>,
    #[serde(flatten)]
    pub found: Found,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Found {
    /// Changes apply would make, per agent, and the builds it would run.
    Changes {
        agents: BTreeMap<String, Summary>,
        builds: Vec<Build>,
    },
    /// `agsync.yaml` does not validate: nothing can be applied.
    Invalid { line: Option<u32>, reason: String },
}

/// Item names (`mcp x`, `skill y`) per kind of change, for one agent.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Summary {
    pub added: Vec<String>,
    pub changed: Vec<String>,
    pub removed: Vec<String>,
    pub conflicts: Vec<String>,
}

impl Summary {
    fn count(&self) -> usize {
        self.added.len() + self.changed.len() + self.removed.len()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Build {
    pub name: String,
    pub repo: String,
    pub sha: String,
}

impl Pending {
    /// What apply would change and the manual installs `--adopt` would replace, or
    /// `None` when there is neither.
    pub fn changes(report: &Report, checked_at: String) -> Option<Pending> {
        let mut agents = BTreeMap::new();
        for plan in &report.agents {
            let mut summary = Summary::default();
            for (kind, change) in plan.visible() {
                let item = format!("{kind} {}", change.name);
                match change.action {
                    Action::Add | Action::Adopt => summary.added.push(item),
                    Action::Update | Action::Overwrite => summary.changed.push(item),
                    Action::Remove | Action::Forget => summary.removed.push(item),
                    Action::Conflict => summary.conflicts.push(item),
                    Action::Keep => {}
                }
            }
            if summary != Summary::default() {
                agents.insert(plan.agent.to_string(), summary);
            }
        }
        if !report.has_pending() && agents.is_empty() {
            return None;
        }
        let builds = report
            .builds
            .iter()
            .map(|job| Build {
                name: job.name.clone(),
                repo: job.repo.clone(),
                sha: job.sha.clone(),
            })
            .collect();
        Some(Pending {
            commit: report.remote.clone(),
            checked_at,
            sources: report.watched.clone(),
            found: Found::Changes { agents, builds },
        })
    }

    /// `agsync.yaml` of `commit` is invalid; `reason` is the validation error.
    pub fn invalid(commit: &str, reason: &str, checked_at: String) -> Pending {
        Pending {
            commit: commit.to_string(),
            checked_at,
            sources: Vec::new(),
            found: Found::Invalid {
                line: line_of(reason),
                reason: reason.to_string(),
            },
        }
    }

    pub fn load(path: &Path) -> Option<Pending> {
        let text = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        crate::atomic::write_atomic(path, text.as_bytes())
    }

    /// Deletes the file; a missing file is fine.
    pub fn clear(path: &Path) -> anyhow::Result<()> {
        match std::fs::remove_file(path) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
            _ => Ok(()),
        }
    }

    /// True when `other` reports the same commit and sources, so it was already notified.
    pub fn same_news(&self, other: &Pending) -> bool {
        self.commit == other.commit && self.sources == other.sources
    }

    /// Changes apply would make; builds count when no item changes.
    pub fn count(&self) -> usize {
        match &self.found {
            Found::Changes { agents, builds } => {
                let items: usize = agents.values().map(Summary::count).sum();
                if items == 0 { builds.len() } else { items }
            }
            Found::Invalid { .. } => 0,
        }
    }

    /// Manual installs with the same name, kind and agent as a declared item.
    pub fn manual(&self) -> usize {
        match &self.found {
            Found::Changes { agents, .. } => agents.values().map(|s| s.conflicts.len()).sum(),
            Found::Invalid { .. } => 0,
        }
    }

    /// Short text for the notification.
    pub fn message(&self) -> String {
        match &self.found {
            Found::Changes { .. } => {
                let (what, hint) = self.news();
                format!("{what}; {hint}")
            }
            Found::Invalid { .. } => format!(
                "agsync.yaml invalido no commit {}; veja `agsync status`",
                self.short()
            ),
        }
    }

    /// One line for the shell hook and the other commands.
    pub fn line(&self) -> String {
        match &self.found {
            Found::Changes { .. } => {
                let (what, hint) = self.news();
                format!("agsync: {what} ({}); {hint}", self.short())
            }
            Found::Invalid { .. } => format!("agsync: {}", self.message()),
        }
    }

    /// What is pending and the command that applies it.
    fn news(&self) -> (String, String) {
        let changes = changes_text(self.count());
        let manual = manual_text(self.manual());
        let apply = "rode `agsync apply`".to_string();
        match (self.count(), self.manual()) {
            (_, 0) => (changes, apply),
            (0, _) => (manual, "`agsync apply --adopt` substitui".to_string()),
            _ => (
                format!("{changes} e {manual}"),
                format!("{apply} (`--adopt` substitui as manuais)"),
            ),
        }
    }

    fn short(&self) -> &str {
        self.commit.get(..7).unwrap_or(&self.commit)
    }
}

/// What the last `check` left for the user: its news and a newer agsync release. Read
/// from `pending.json` and `update.json` only, never from the network.
#[derive(Debug, Serialize)]
pub struct Notices {
    pub pending: Option<Pending>,
    pub newer: Option<Version>,
}

impl Notices {
    pub fn load(dirs: &Dirs) -> Notices {
        Notices {
            pending: Pending::load(&dirs.pending_file()),
            newer: crate::update::recorded(dirs),
        }
    }

    /// The lines `agsync hook` prints, pending first.
    pub fn lines(&self) -> Vec<String> {
        let pending = self.pending.as_ref().map(Pending::line);
        let newer = self.newer.map(crate::update::line);
        pending.into_iter().chain(newer).collect()
    }
}

fn changes_text(count: usize) -> String {
    if count == 1 {
        "1 mudanca pendente".to_string()
    } else {
        format!("{count} mudancas pendentes")
    }
}

fn manual_text(count: usize) -> String {
    if count == 1 {
        "1 instalacao manual com o mesmo nome no agsync.yaml".to_string()
    } else {
        format!("{count} instalacoes manuais com o mesmo nome no agsync.yaml")
    }
}

/// Line number from `linha N` (agsync checks) or `line N` (YAML parser).
fn line_of(reason: &str) -> Option<u32> {
    ["linha ", "line "].iter().find_map(|word| {
        let (_, rest) = reason.split_once(word)?;
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        digits.parse().ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn changes(added: usize, builds: usize) -> Pending {
        let summary = Summary {
            added: (0..added).map(|i| format!("mcp m{i}")).collect(),
            ..Summary::default()
        };
        let build = Build {
            name: "b".into(),
            repo: "r".into(),
            sha: "s".into(),
        };
        Pending {
            commit: "abcdef1234567890".into(),
            checked_at: "t".into(),
            sources: Vec::new(),
            found: Found::Changes {
                agents: BTreeMap::from([("claude".to_string(), summary)]),
                builds: vec![build; builds],
            },
        }
    }

    #[test]
    fn lines_for_changes_and_errors() {
        assert_eq!(
            changes(3, 1).line(),
            "agsync: 3 mudancas pendentes (abcdef1); rode `agsync apply`"
        );
        assert_eq!(
            changes(1, 0).message(),
            "1 mudanca pendente; rode `agsync apply`"
        );
        assert_eq!(changes(0, 2).count(), 2);
        let bad = Pending::invalid("abcdef1234", "agsync.yaml linha 4: mcp 'x': y", "t".into());
        assert_eq!(
            bad.line(),
            "agsync: agsync.yaml invalido no commit abcdef1; veja `agsync status`"
        );
        assert_eq!(bad.count(), 0);
    }

    #[test]
    fn manual_installs_are_counted_apart_with_the_adopt_hint() {
        let mut only_manual = changes(0, 0);
        let Found::Changes { agents, .. } = &mut only_manual.found else {
            unreachable!()
        };
        let claude = agents.get_mut("claude").unwrap();
        claude.conflicts = vec!["mcp x".into(), "skill s".into()];
        assert_eq!(only_manual.count(), 0);
        assert_eq!(only_manual.manual(), 2);
        assert_eq!(
            only_manual.line(),
            "agsync: 2 instalacoes manuais com o mesmo nome no agsync.yaml (abcdef1); `agsync apply --adopt` substitui"
        );

        let mut both = changes(1, 0);
        let Found::Changes { agents, .. } = &mut both.found else {
            unreachable!()
        };
        agents.get_mut("claude").unwrap().conflicts = vec!["plugin p".into()];
        assert_eq!(
            both.message(),
            "1 mudanca pendente e 1 instalacao manual com o mesmo nome no agsync.yaml; rode `agsync apply` (`--adopt` substitui as manuais)"
        );
    }

    #[test]
    fn line_number_from_either_message() {
        assert_eq!(line_of("agsync.yaml linha 12: skill: x"), Some(12));
        assert_eq!(
            line_of("agsync.yaml: unknown field `x` at line 5 column 3"),
            Some(5)
        );
        assert_eq!(line_of("agsync.yaml: version 2 nao suportada"), None);
    }

    #[test]
    fn round_trips_and_clears() {
        let tmp = crate::tmp::TempDir::new("agsync-test").unwrap();
        let file = tmp.path().join("pending.json");
        assert!(Pending::load(&file).is_none());
        let p = changes(2, 1);
        p.save(&file).unwrap();
        let text = std::fs::read_to_string(&file).unwrap();
        assert!(text.contains("\"kind\": \"changes\""), "{text}");
        assert_eq!(Pending::load(&file).unwrap(), p);
        let bad = Pending::invalid("c", "linha 3: x", "t".into());
        bad.save(&file).unwrap();
        assert_eq!(Pending::load(&file).unwrap(), bad);
        Pending::clear(&file).unwrap();
        Pending::clear(&file).unwrap();
        assert!(!file.exists());
    }
}
