//! Removes source checkouts and cached skill folders no state entry uses anymore.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::Context;

use crate::state::State;

/// `(name, id)` pairs of a two-level cache: `<root>/<name>/<id>/`.
pub type Used = BTreeSet<(String, String)>;

/// Source checkouts in use: `(item name, sha)` of every tracked entry with a sha.
pub fn used_sources(state: &State) -> Used {
    state
        .agents
        .values()
        .flat_map(|a| a.mcps.iter().chain(&a.skills))
        .filter_map(|(name, t)| t.sha.clone().map(|sha| (name.clone(), sha)))
        .collect()
}

/// Skill cache folders in use: `(skill name, content hash)` of every tracked skill.
pub fn used_skill_cache(state: &State) -> Used {
    state
        .agents
        .values()
        .flat_map(|a| &a.skills)
        .map(|(name, t)| (name.clone(), t.hash.clone()))
        .collect()
}

/// Entries of `found` that `used` does not mention.
pub fn stale(found: &[(String, String)], used: &Used) -> Vec<(String, String)> {
    found
        .iter()
        .filter(|pair| !used.contains(*pair))
        .cloned()
        .collect()
}

/// Deletes stale `<root>/<name>/<id>` entries, then empty `<name>` folders.
pub fn clean(root: &Path, used: &Used) -> anyhow::Result<Vec<PathBuf>> {
    let found = list(root)?;
    let mut removed = Vec::new();
    for (name, id) in stale(&found, used) {
        let path = root.join(&name).join(&id);
        remove(&path)?;
        removed.push(path);
    }
    for (name, _) in &found {
        let dir = root.join(name);
        let empty = std::fs::read_dir(&dir).is_ok_and(|mut d| d.next().is_none());
        if empty {
            std::fs::remove_dir(&dir).with_context(|| format!("remover {}", dir.display()))?;
        }
    }
    Ok(removed)
}

fn list(root: &Path) -> anyhow::Result<Vec<(String, String)>> {
    let mut found = Vec::new();
    let Ok(names) = std::fs::read_dir(root) else {
        return Ok(found);
    };
    for name in names {
        let name = name?;
        if !name.path().is_dir() {
            continue;
        }
        let ids = std::fs::read_dir(name.path())
            .with_context(|| format!("ler {}", name.path().display()))?;
        for id in ids {
            let pair = (
                name.file_name().to_string_lossy().to_string(),
                id?.file_name().to_string_lossy().to_string(),
            );
            found.push(pair);
        }
    }
    Ok(found)
}

fn remove(path: &Path) -> anyhow::Result<()> {
    let meta = std::fs::symlink_metadata(path)?;
    let result = if meta.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    result.with_context(|| format!("remover {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::Tracked;
    use crate::tmp::TempDir;

    fn pair(a: &str, b: &str) -> (String, String) {
        (a.to_string(), b.to_string())
    }

    fn tracked(hash: &str, sha: Option<&str>) -> Tracked {
        Tracked {
            hash: hash.into(),
            mode: None,
            sha: sha.map(String::from),
            plugin: None,
        }
    }

    #[test]
    fn rule_keeps_only_pairs_in_state() {
        let mut state = State::default();
        let claude = state.agent_mut("claude");
        claude.mcps.insert("srv".into(), tracked("h1", Some("s1")));
        claude.mcps.insert("plain".into(), tracked("h2", None));
        claude.skills.insert("sk".into(), tracked("h3", Some("s2")));
        claude.skills.insert("local".into(), tracked("h4", None));

        let sources = used_sources(&state);
        assert_eq!(sources, Used::from([pair("srv", "s1"), pair("sk", "s2")]));
        let cache = used_skill_cache(&state);
        assert_eq!(cache, Used::from([pair("sk", "h3"), pair("local", "h4")]));

        let found = [pair("srv", "s1"), pair("srv", "old"), pair("gone", "s9")];
        assert_eq!(
            stale(&found, &sources),
            [pair("srv", "old"), pair("gone", "s9")]
        );
    }

    #[test]
    fn clean_removes_stale_dirs_and_empty_names() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let root = tmp.path();
        for dir in ["srv/s1", "srv/old", "srv/s1.tmp", "gone/s9"] {
            std::fs::create_dir_all(root.join(dir)).unwrap();
            std::fs::write(root.join(dir).join("f"), "x").unwrap();
        }
        let removed = clean(root, &Used::from([pair("srv", "s1")])).unwrap();
        assert_eq!(removed.len(), 3);
        assert!(root.join("srv/s1/f").is_file());
        assert!(!root.join("srv/old").exists());
        assert!(!root.join("srv/s1.tmp").exists());
        assert!(!root.join("gone").exists());
        assert!(
            clean(&root.join("missing"), &Used::new())
                .unwrap()
                .is_empty()
        );
    }
}
