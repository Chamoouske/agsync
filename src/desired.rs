//! What each agent should have, from the manifest and this machine's profiles.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::hash::hash_dir;
use crate::manifest::{GIT_SOURCE_SKIP, Manifest, selected, targets};
use crate::mcp::McpSpec;

/// A skill folder inside the snapshot, with its content hash.
#[derive(Debug, Clone)]
pub struct SkillSrc {
    pub dir: PathBuf,
    pub hash: String,
}

/// Entries one agent should have.
#[derive(Debug, Default)]
pub struct Desired {
    pub mcps: BTreeMap<String, McpSpec>,
    pub skills: BTreeMap<String, SkillSrc>,
}

impl Desired {
    /// Selected entries for `agent`. Items with a git source are left out.
    pub fn for_agent(
        manifest: &Manifest,
        root: &Path,
        active: &[String],
        agent: &str,
    ) -> anyhow::Result<Desired> {
        let mut desired = Desired::default();
        for (name, mcp) in &manifest.mcps {
            let mcp = &mcp.value;
            let wanted = selected(&mcp.profiles, active) && targets(&mcp.agents, agent);
            if wanted && !mcp.has_git_source() {
                desired
                    .mcps
                    .insert(name.clone(), McpSpec::from_manifest(mcp));
            }
        }
        for skill in &manifest.skills {
            let skill = &skill.value;
            let wanted = selected(&skill.profiles, active) && targets(&skill.agents, agent);
            let Some(path) = &skill.path else {
                continue;
            };
            if wanted && skill.git.is_none() {
                let dir = root.join(path);
                let hash = hash_dir(&dir)?;
                desired.skills.insert(skill.name(), SkillSrc { dir, hash });
            }
        }
        Ok(desired)
    }

    pub fn mcp_hashes(&self) -> BTreeMap<String, String> {
        self.mcps
            .iter()
            .map(|(name, spec)| (name.clone(), spec.hash()))
            .collect()
    }

    pub fn skill_hashes(&self) -> BTreeMap<String, String> {
        self.skills
            .iter()
            .map(|(name, src)| (name.clone(), src.hash.clone()))
            .collect()
    }
}

/// Selected items skipped in this milestone, with the reason.
pub fn skipped_git_items(manifest: &Manifest, active: &[String]) -> Vec<String> {
    let mut skipped = Vec::new();
    for (name, mcp) in &manifest.mcps {
        if mcp.value.has_git_source() && selected(&mcp.value.profiles, active) {
            skipped.push(format!("mcp {name}: {GIT_SOURCE_SKIP}"));
        }
    }
    for skill in &manifest.skills {
        if skill.value.git.is_some() && selected(&skill.value.profiles, active) {
            skipped.push(format!("skill {}: {GIT_SOURCE_SKIP}", skill.value.name()));
        }
    }
    skipped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmp::TempDir;

    const YAML: &str = r#"
version: 1
profiles: [work, personal]
mcps:
  all:
    command: a
  work_only:
    profiles: [work]
    command: b
  personal_only:
    profiles: [personal]
    command: c
  codex_only:
    agents: [codex]
    command: d
  built:
    command: e
    source: {git: "https://x", ref: v1}
skills:
  - path: skills/hello
  - path: skills/home
    profiles: [personal]
  - git: https://x/remote-skill.git
    ref: v2
"#;

    fn setup() -> (TempDir, Manifest) {
        let tmp = TempDir::new("agsync-test").unwrap();
        for name in ["hello", "home"] {
            let dir = tmp.path().join("skills").join(name);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("SKILL.md"), name).unwrap();
        }
        (tmp, Manifest::parse(YAML).unwrap())
    }

    #[test]
    fn selects_by_profile_and_agent() {
        let (tmp, m) = setup();
        let active = vec!["work".to_string()];
        let claude = Desired::for_agent(&m, tmp.path(), &active, "claude").unwrap();
        let mcps: Vec<_> = claude.mcps.keys().map(String::as_str).collect();
        assert_eq!(mcps, ["all", "work_only"]);
        let skills: Vec<_> = claude.skills.keys().map(String::as_str).collect();
        assert_eq!(skills, ["hello"]);

        let codex = Desired::for_agent(&m, tmp.path(), &active, "codex").unwrap();
        assert!(codex.mcps.contains_key("codex_only"));
    }

    #[test]
    fn several_active_profiles_add_up() {
        let (tmp, m) = setup();
        let active = vec!["work".to_string(), "personal".to_string()];
        let d = Desired::for_agent(&m, tmp.path(), &active, "claude").unwrap();
        assert_eq!(d.mcps.len(), 3);
        assert_eq!(d.skills.len(), 2);
    }

    #[test]
    fn git_items_are_skipped_with_reason() {
        let (_tmp, m) = setup();
        let skipped = skipped_git_items(&m, &["work".to_string()]);
        assert_eq!(
            skipped,
            [
                "mcp built: fonte git: marco 3",
                "skill remote-skill: fonte git: marco 3"
            ]
        );
    }
}
