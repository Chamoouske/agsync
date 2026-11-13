//! What each agent should have, from the manifest and this machine's profiles.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::deliver::{McpWant, missing_hint};
use crate::hash::hash_dir;
use crate::manifest::{GIT_SOURCE_SKIP, Manifest, selected, targets};
use crate::mcp::McpSpec;
use crate::plan::Want;
use crate::secret::Secrets;
use crate::state::{LinkMode, Tracked, state_key};

/// A skill folder (in the snapshot or a git source checkout), with its content hash.
#[derive(Debug, Clone)]
pub struct SkillSrc {
    pub dir: PathBuf,
    pub hash: String,
    /// Commit of the git source, if any.
    pub sha: Option<String>,
}

impl SkillSrc {
    pub fn tracked(&self, mode: Option<LinkMode>) -> Tracked {
        Tracked {
            hash: self.hash.clone(),
            mode,
            sha: self.sha.clone(),
        }
    }

    pub fn want(&self) -> Want {
        Want {
            key: state_key(&self.hash, self.sha.as_deref()),
            write: self.hash.clone(),
        }
    }
}

/// Inputs shared by every agent.
pub struct Context<'a> {
    pub manifest: &'a Manifest,
    /// Config repo snapshot.
    pub root: &'a Path,
    pub active: &'a [String],
    pub secrets: &'a Secrets,
}

/// Entries one agent should have.
#[derive(Debug, Default)]
pub struct Desired {
    pub mcps: BTreeMap<String, McpWant>,
    pub skills: BTreeMap<String, SkillSrc>,
    /// Selected names left untouched this time (state and agent entry kept as they are).
    pub held: BTreeSet<String>,
    /// Why each held item was skipped.
    pub skipped: Vec<String>,
}

impl Desired {
    /// Selected entries for `agent`. Items with a git source are left out.
    /// `expands_refs`: the agent expands `${VAR}` from its own environment.
    pub fn for_agent(ctx: &Context, agent: &str, expands_refs: bool) -> anyhow::Result<Desired> {
        let mut desired = Desired::default();
        for (name, mcp) in &ctx.manifest.mcps {
            let mcp = &mcp.value;
            let wanted = selected(&mcp.profiles, ctx.active) && targets(&mcp.agents, agent);
            if !wanted || mcp.has_git_source() {
                continue;
            }
            let shown = McpSpec::from_manifest(mcp);
            match McpWant::new(shown, ctx.secrets, expands_refs, None) {
                Ok(want) => {
                    desired.mcps.insert(name.clone(), want);
                }
                Err(missing) => desired.hold(name, agent, &missing),
            }
        }
        for skill in &ctx.manifest.skills {
            let skill = &skill.value;
            let wanted = selected(&skill.profiles, ctx.active) && targets(&skill.agents, agent);
            let Some(path) = &skill.path else {
                continue;
            };
            if wanted && skill.git.is_none() {
                let dir = ctx.root.join(path);
                let hash = hash_dir(&dir)?;
                let src = SkillSrc {
                    dir,
                    hash,
                    sha: None,
                };
                desired.skills.insert(skill.name(), src);
            }
        }
        Ok(desired)
    }

    fn hold(&mut self, name: &str, agent: &str, missing: &[String]) {
        self.held.insert(name.to_string());
        for var in missing {
            self.skipped
                .push(format!("{agent}: mcp {name}: {}", missing_hint(var)));
        }
    }

    pub fn mcp_wants(&self) -> BTreeMap<String, Want> {
        self.mcps
            .iter()
            .map(|(name, want)| (name.clone(), want.want()))
            .collect()
    }

    pub fn skill_wants(&self) -> BTreeMap<String, Want> {
        self.skills
            .iter()
            .map(|(name, src)| (name.clone(), src.want()))
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

    fn desired(tmp: &TempDir, m: &Manifest, active: &[String], agent: &str) -> Desired {
        let secrets = Secrets::default();
        let ctx = Context {
            manifest: m,
            root: tmp.path(),
            active,
            secrets: &secrets,
        };
        Desired::for_agent(&ctx, agent, true).unwrap()
    }

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
        let claude = desired(&tmp, &m, &active, "claude");
        let mcps: Vec<_> = claude.mcps.keys().map(String::as_str).collect();
        assert_eq!(mcps, ["all", "work_only"]);
        let skills: Vec<_> = claude.skills.keys().map(String::as_str).collect();
        assert_eq!(skills, ["hello"]);

        let codex = desired(&tmp, &m, &active, "codex");
        assert!(codex.mcps.contains_key("codex_only"));
    }

    #[test]
    fn several_active_profiles_add_up() {
        let (tmp, m) = setup();
        let active = vec!["work".to_string(), "personal".to_string()];
        let d = desired(&tmp, &m, &active, "claude");
        assert_eq!(d.mcps.len(), 3);
        assert_eq!(d.skills.len(), 2);
    }

    #[test]
    fn mcp_with_missing_secret_is_held() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let yaml = "version: 1\nmcps:\n  a:\n    command: x\n    env: {T: \"${AGSYNC_TEST_NEVER_SET}\"}\n  b:\n    command: y\n";
        let m = Manifest::parse(yaml).unwrap();
        let d = desired(&tmp, &m, &[], "claude");
        assert!(d.mcps.contains_key("b"));
        assert!(!d.mcps.contains_key("a"));
        assert!(d.held.contains("a"));
        assert!(
            d.skipped[0].contains("AGSYNC_TEST_NEVER_SET"),
            "{:?}",
            d.skipped
        );
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
