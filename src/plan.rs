//! Merge rules: what to do with each entry, given desired, tracked and current hashes.

use std::collections::{BTreeMap, BTreeSet};

use serde::Serialize;

/// Decision for one named entry of one agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// New entry, name free in the agent config.
    Add,
    /// Tracked, untouched by hand, desired content changed.
    Update,
    /// Tracked but changed or deleted by hand: manual drift, overwritten with backup.
    Overwrite,
    /// Manual entry with the same name, kind and agent, replaced by the config repo
    /// definition and tracked from now on, because of `--adopt`.
    Adopt,
    /// Manual entry with the same name, kind and agent; left alone without `--adopt`.
    Conflict,
    /// Tracked, no longer desired, still present: removed.
    Remove,
    /// Tracked, no longer desired, already gone: dropped from state.
    Forget,
    /// Already as desired.
    Keep,
}

impl Action {
    /// True if the agent config is written.
    pub fn writes(self) -> bool {
        matches!(
            self,
            Action::Add | Action::Update | Action::Overwrite | Action::Adopt | Action::Remove
        )
    }

    pub fn label(self) -> &'static str {
        match self {
            Action::Add => "adicionar",
            Action::Update => "atualizar",
            Action::Overwrite => "desvio manual, sobrescrever com backup",
            Action::Adopt => "assumir instalacao manual, substituida pelo agsync.yaml (--adopt)",
            Action::Conflict => {
                "conflito com instalacao manual de mesmo nome, nao sera alterada (use --adopt para substituir)"
            }
            Action::Remove => "remover (saiu do agsync.yaml)",
            Action::Forget => "esquecer (ja removida fora do agsync)",
            Action::Keep => "sem mudanca",
        }
    }
}

/// What agsync wants for one entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Want {
    /// Identity kept in state: changes when the config repo changes, never with secret values.
    pub key: String,
    /// Hash of what would be written to the agent now.
    pub write: String,
}

/// One planned change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub name: String,
    pub action: Action,
}

/// Plans every name seen in `desired`, `tracked` (state keys) or `current` (agent config
/// hashes). Names only in `current` are manual entries and never appear in the result.
pub fn plan(
    desired: &BTreeMap<String, Want>,
    tracked: &BTreeMap<String, String>,
    current: &BTreeMap<String, String>,
    adopt: bool,
) -> Vec<Change> {
    let names: BTreeSet<&String> = desired.keys().chain(tracked.keys()).collect();
    names
        .into_iter()
        .map(|name| Change {
            name: name.clone(),
            action: decide(
                desired.get(name),
                tracked.get(name),
                current.get(name),
                adopt,
            ),
        })
        .collect()
}

fn decide(
    desired: Option<&Want>,
    tracked: Option<&String>,
    current: Option<&String>,
    adopt: bool,
) -> Action {
    match (desired, tracked, current) {
        (Some(_), None, None) => Action::Add,
        (Some(_), None, Some(_)) if adopt => Action::Adopt,
        (Some(_), None, Some(_)) => Action::Conflict,
        (Some(d), Some(t), Some(c)) if *t == d.key && *c == d.write => Action::Keep,
        (Some(d), Some(t), _) if *t != d.key => Action::Update,
        (Some(_), Some(_), _) => Action::Overwrite,
        (None, Some(_), Some(_)) => Action::Remove,
        (None, Some(_), None) => Action::Forget,
        (None, None, _) => Action::Keep,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn wants(pairs: &[(&str, &str)]) -> BTreeMap<String, Want> {
        pairs
            .iter()
            .map(|(k, v)| {
                let want = Want {
                    key: v.to_string(),
                    write: v.to_string(),
                };
                (k.to_string(), want)
            })
            .collect()
    }

    fn one(desired: &[(&str, &str)], tracked: &[(&str, &str)], current: &[(&str, &str)]) -> Action {
        let changes = plan(&wants(desired), &map(tracked), &map(current), false);
        assert_eq!(changes.len(), 1, "{changes:?}");
        changes[0].action
    }

    #[test]
    fn new_entry_is_added() {
        assert_eq!(one(&[("a", "1")], &[], &[]), Action::Add);
    }

    #[test]
    fn untouched_entry_is_kept_or_updated() {
        assert_eq!(
            one(&[("a", "1")], &[("a", "1")], &[("a", "1")]),
            Action::Keep
        );
        assert_eq!(
            one(&[("a", "2")], &[("a", "1")], &[("a", "1")]),
            Action::Update
        );
    }

    #[test]
    fn manual_drift_is_overwritten() {
        assert_eq!(
            one(&[("a", "1")], &[("a", "1")], &[("a", "x")]),
            Action::Overwrite
        );
        assert_eq!(one(&[("a", "1")], &[("a", "1")], &[]), Action::Overwrite);
    }

    #[test]
    fn repo_change_wins_over_manual_drift() {
        assert_eq!(
            one(&[("a", "2")], &[("a", "1")], &[("a", "x")]),
            Action::Update
        );
    }

    #[test]
    fn new_key_is_an_update_even_if_written_content_matches() {
        assert_eq!(
            one(&[("a", "2")], &[("a", "1")], &[("a", "2")]),
            Action::Update
        );
    }

    #[test]
    fn key_and_written_hash_are_compared_separately() {
        let want = Want {
            key: "k".into(),
            write: "w".into(),
        };
        let desired = BTreeMap::from([("a".to_string(), want)]);
        let keep = plan(&desired, &map(&[("a", "k")]), &map(&[("a", "w")]), false);
        assert_eq!(keep[0].action, Action::Keep);
        let drift = plan(&desired, &map(&[("a", "k")]), &map(&[("a", "k")]), false);
        assert_eq!(drift[0].action, Action::Overwrite);
    }

    #[test]
    fn entry_gone_from_yaml_is_removed() {
        assert_eq!(one(&[], &[("a", "1")], &[("a", "1")]), Action::Remove);
        assert_eq!(one(&[], &[("a", "1")], &[("a", "x")]), Action::Remove);
        assert_eq!(one(&[], &[("a", "1")], &[]), Action::Forget);
    }

    #[test]
    fn name_clash_with_manual_entry_is_conflict_unless_adopt() {
        assert_eq!(one(&[("a", "1")], &[], &[("a", "x")]), Action::Conflict);
        assert_eq!(one(&[("a", "1")], &[], &[("a", "1")]), Action::Conflict);
        let adopted = plan(&wants(&[("a", "1")]), &map(&[]), &map(&[("a", "x")]), true);
        assert_eq!(adopted[0].action, Action::Adopt);
    }

    #[test]
    fn manual_entries_outside_state_are_never_touched() {
        let changes = plan(&wants(&[]), &map(&[]), &map(&[("manual", "x")]), true);
        assert!(changes.is_empty());
    }

    #[test]
    fn plans_names_in_sorted_order() {
        let changes = plan(
            &wants(&[("b", "1"), ("a", "1")]),
            &map(&[("c", "1")]),
            &map(&[("c", "1")]),
            false,
        );
        let names: Vec<_> = changes.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["a", "b", "c"]);
    }
}
