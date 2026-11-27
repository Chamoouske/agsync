//! What each agent should have, from the manifest and this machine's profiles.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use crate::deliver::{KeepsRef, McpWant, missing_hint};
use crate::hash::hash_dir;
use crate::manifest::{Manifest, Mcp, selected, targets};
use crate::mcp::McpSpec;
use crate::plan::Want;
use crate::plugin::PluginSpec;
use crate::resolved::Resolved;
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
            plugin: None,
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
    pub sources: &'a Resolved,
}

/// Entries one agent should have.
#[derive(Debug, Default)]
pub struct Desired {
    pub mcps: BTreeMap<String, McpWant>,
    pub skills: BTreeMap<String, SkillSrc>,
    pub plugins: BTreeMap<String, PluginSpec>,
    /// Selected MCPs left untouched this time (state and agent entry kept as they are).
    pub held_mcps: BTreeSet<String>,
    /// Selected skills left untouched this time.
    pub held_skills: BTreeSet<String>,
    /// Why each MCP held for this agent was skipped (source problems are in [`Resolved`]).
    pub skipped: Vec<String>,
}

impl Desired {
    /// Selected entries for `agent`.
    /// `keeps_ref`: the agent's rule for where a `${VAR}` reference may stay.
    pub fn for_agent(ctx: &Context, agent: &str, keeps_ref: KeepsRef) -> anyhow::Result<Desired> {
        let mut desired = Desired {
            held_mcps: ctx.sources.held_mcps.clone(),
            held_skills: ctx.sources.held_skills.clone(),
            ..Desired::default()
        };
        for (name, mcp) in &ctx.manifest.mcps {
            let mcp = &mcp.value;
            let wanted = selected(&mcp.profiles, ctx.active) && targets(&mcp.agents, agent);
            if !wanted || desired.held_mcps.contains(name) {
                continue;
            }
            let (shown, sha) = mcp_spec(name, mcp, ctx.sources);
            match McpWant::new(shown, ctx.secrets, keeps_ref, sha) {
                Ok(want) => {
                    desired.mcps.insert(name.clone(), want);
                }
                Err(missing) => desired.hold_mcp(name, agent, &missing),
            }
        }
        for skill in &ctx.manifest.skills {
            let skill = &skill.value;
            let name = skill.name();
            let wanted = selected(&skill.profiles, ctx.active) && targets(&skill.agents, agent);
            if !wanted || desired.held_skills.contains(&name) {
                continue;
            }
            let (dir, sha) = match ctx.sources.skills.get(&name) {
                Some(src) if skill.git.is_some() => (src.dir.clone(), Some(src.sha.clone())),
                _ => (ctx.root.join(skill.path.as_deref().unwrap_or("")), None),
            };
            let hash = hash_dir(&dir)?;
            desired.skills.insert(name, SkillSrc { dir, hash, sha });
        }
        for plugin in &ctx.manifest.plugins {
            let plugin = &plugin.value;
            let wanted = selected(&plugin.profiles, ctx.active) && targets(&plugin.agents, agent);
            if !wanted {
                continue;
            }
            let spec = PluginSpec::new(
                plugin.name.clone(),
                plugin.marketplace.git.clone(),
                plugin.marketplace.git_ref.clone(),
            );
            desired.plugins.insert(plugin.name.clone(), spec);
        }
        Ok(desired)
    }

    fn hold_mcp(&mut self, name: &str, agent: &str, missing: &[String]) {
        self.held_mcps.insert(name.to_string());
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

    pub fn plugin_wants(&self) -> BTreeMap<String, Want> {
        self.plugins
            .iter()
            .map(|(name, spec)| (name.clone(), spec.want()))
            .collect()
    }
}

/// Entry as written in the manifest, with `{dir}` pointing at the source checkout.
fn mcp_spec(name: &str, mcp: &Mcp, sources: &Resolved) -> (McpSpec, Option<String>) {
    let spec = McpSpec::from_manifest(mcp);
    match sources.mcps.get(name) {
        Some(src) if mcp.has_git_source() => (spec.with_dir(&src.dir), Some(src.sha.clone())),
        _ => (spec, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolved::{McpSource, SkillSource};
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
    command: "{dir}/srv"
    source: {git: "https://x", ref: v1}
skills:
  - path: skills/hello
  - path: skills/home
    profiles: [personal]
  - git: https://x/remote.git
    ref: v2
    path: sub/remote-skill
plugins:
  - name: all-plugin
    marketplace: {git: https://x/market.git, ref: v1}
  - name: work-plugin
    profiles: [work]
    marketplace: {git: https://x/market.git, ref: v1}
  - name: claude-plugin
    agents: [claude]
    marketplace: {git: https://x/market.git, ref: v1}
"#;

    fn setup() -> (TempDir, Manifest) {
        let tmp = TempDir::new("agsync-test").unwrap();
        for name in ["skills/hello", "skills/home", "checkout/sub/remote-skill"] {
            let dir = tmp.path().join(name);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join("SKILL.md"), name).unwrap();
        }
        (tmp, Manifest::parse(YAML).unwrap())
    }

    fn sources(tmp: &TempDir) -> Resolved {
        let mut resolved = Resolved::default();
        let mcp = McpSource {
            sha: "a".repeat(40),
            dir: PathBuf::from("/data/sources/built/aaa"),
            job: None,
        };
        resolved.mcps.insert("built".into(), mcp);
        let skill = SkillSource {
            sha: "b".repeat(40),
            dir: tmp.path().join("checkout/sub/remote-skill"),
        };
        resolved.skills.insert("remote-skill".into(), skill);
        resolved
    }

    fn desired(
        tmp: &TempDir,
        m: &Manifest,
        src: &Resolved,
        active: &[&str],
        agent: &str,
    ) -> Desired {
        let secrets = Secrets::default();
        let active: Vec<String> = active.iter().map(|s| s.to_string()).collect();
        let ctx = Context {
            manifest: m,
            root: tmp.path(),
            active: &active,
            secrets: &secrets,
            sources: src,
        };
        Desired::for_agent(&ctx, agent, &|_| true).unwrap()
    }

    fn names<T>(map: &BTreeMap<String, T>) -> Vec<&str> {
        map.keys().map(String::as_str).collect()
    }

    #[test]
    fn selects_by_profile_and_agent() {
        let (tmp, m) = setup();
        let src = sources(&tmp);
        let claude = desired(&tmp, &m, &src, &["work"], "claude");
        assert_eq!(names(&claude.mcps), ["all", "built", "work_only"]);
        assert_eq!(names(&claude.skills), ["hello", "remote-skill"]);
        assert_eq!(
            names(&claude.plugins),
            ["all-plugin", "claude-plugin", "work-plugin"]
        );
        assert_eq!(
            claude.plugins["all-plugin"].want().key,
            claude.plugin_wants()["all-plugin"].key
        );

        let codex = desired(&tmp, &m, &src, &["work"], "codex");
        assert!(codex.mcps.contains_key("codex_only"));
        assert_eq!(names(&codex.plugins), ["all-plugin", "work-plugin"]);

        let both = desired(&tmp, &m, &src, &["work", "personal"], "claude");
        assert_eq!(both.mcps.len(), 4);
        assert_eq!(both.skills.len(), 3);
        assert_eq!(both.plugins.len(), 3);
    }

    #[test]
    fn git_items_use_checkout_and_sha() {
        let (tmp, m) = setup();
        let src = sources(&tmp);
        let d = desired(&tmp, &m, &src, &[], "claude");
        let built = &d.mcps["built"];
        let json = built.write.to_json();
        assert!(
            json.contains("aaa/srv") || json.contains("aaa\\\\srv"),
            "{json}"
        );
        assert_eq!(built.sha, Some("a".repeat(40)));
        assert!(built.want().key.ends_with(&"a".repeat(40)));
        let skill = &d.skills["remote-skill"];
        assert_eq!(skill.sha, Some("b".repeat(40)));
        assert_eq!(skill.dir, tmp.path().join("checkout/sub/remote-skill"));
    }

    #[test]
    fn held_sources_are_left_out() {
        let (tmp, m) = setup();
        let mut src = sources(&tmp);
        src.mcps.clear();
        src.held_mcps.insert("built".into());
        src.skills.clear();
        src.held_skills.insert("remote-skill".into());
        let d = desired(&tmp, &m, &src, &[], "claude");
        assert!(!d.mcps.contains_key("built"));
        assert!(d.held_mcps.contains("built"));
        assert!(!d.skills.contains_key("remote-skill"));
        assert!(d.held_skills.contains("remote-skill"));
    }

    #[test]
    fn mcp_with_missing_secret_is_held() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let yaml = "version: 1\nmcps:\n  a:\n    command: x\n    env: {T: \"${AGSYNC_TEST_NEVER_SET}\"}\n  b:\n    command: y\n";
        let m = Manifest::parse(yaml).unwrap();
        let d = desired(&tmp, &m, &Resolved::default(), &[], "claude");
        assert!(d.mcps.contains_key("b"));
        assert!(!d.mcps.contains_key("a"));
        assert!(d.held_mcps.contains("a"));
        assert!(
            d.skipped[0].contains("AGSYNC_TEST_NEVER_SET"),
            "{:?}",
            d.skipped
        );
    }
}
