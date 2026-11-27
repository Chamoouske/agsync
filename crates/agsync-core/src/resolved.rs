//! Git sources of the active items, resolved once per run.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::git;
use crate::manifest::{Manifest, selected};
use crate::paths::Dirs;
use crate::source::{self, BuildJob};

/// An MCP server source: commit, checkout folder and the build still to run, if any.
#[derive(Debug, Clone)]
pub struct McpSource {
    pub sha: String,
    pub dir: PathBuf,
    pub job: Option<BuildJob>,
}

/// A skill from a git source: commit and the skill folder inside its checkout.
#[derive(Debug, Clone)]
pub struct SkillSource {
    pub sha: String,
    pub dir: PathBuf,
}

/// A git source followed by branch or tag, and the commit it pointed to.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Watched {
    pub url: String,
    #[serde(rename = "ref")]
    pub git_ref: String,
    pub sha: String,
}

impl Watched {
    /// `None` when `git_ref` is a full SHA: it never moves.
    pub fn new(url: &str, git_ref: &str, sha: &str) -> Option<Watched> {
        let moves = !git::is_full_sha(git_ref);
        moves.then(|| Watched {
            url: url.to_string(),
            git_ref: git_ref.to_string(),
            sha: sha.to_string(),
        })
    }
}

/// Sources by item name, plus the items held back and why.
#[derive(Debug, Default)]
pub struct Resolved {
    pub mcps: BTreeMap<String, McpSource>,
    pub skills: BTreeMap<String, SkillSource>,
    pub held_mcps: BTreeSet<String>,
    pub held_skills: BTreeSet<String>,
    pub skipped: Vec<String>,
    /// Sources that can move without a new config commit, sorted and without repeats.
    pub watched: Vec<Watched>,
}

impl Resolved {
    /// Resolves refs to SHAs for items of active profiles. MCPs are not cloned here (that
    /// happens with the build, after confirmation); skills are checked out, never built.
    pub fn new(manifest: &Manifest, active: &[String], dirs: &Dirs) -> Resolved {
        let mut resolved = Resolved::default();
        let sources = std::path::absolute(dirs.sources()).unwrap_or_else(|_| dirs.sources());
        for (name, mcp) in &manifest.mcps {
            let mcp = &mcp.value;
            let Some(src) = &mcp.source else {
                continue;
            };
            if !selected(&mcp.profiles, active) {
                continue;
            }
            let git_ref = src.git_ref.clone().unwrap_or_default();
            match mcp_source(
                name,
                &src.git,
                &git_ref,
                &src.requires,
                &src.build,
                &sources,
            ) {
                Ok(found) => {
                    resolved.watch(&src.git, &git_ref, &found.sha);
                    resolved.mcps.insert(name.clone(), found);
                }
                Err(reason) => {
                    resolved.held_mcps.insert(name.clone());
                    resolved.skipped.push(format!("mcp {name}: {reason}"));
                }
            }
        }
        let mut fetched = Fetched::default();
        for skill in &manifest.skills {
            let skill = &skill.value;
            let (Some(url), Some(git_ref), Some(path)) = (&skill.git, &skill.git_ref, &skill.path)
            else {
                continue;
            };
            if !selected(&skill.profiles, active) {
                continue;
            }
            let name = skill.name();
            match fetched.skill_source(&name, url, git_ref, path, &sources) {
                Ok(found) => {
                    resolved.watch(url, git_ref, &found.sha);
                    resolved.skills.insert(name, found);
                }
                Err(reason) => {
                    resolved.skipped.push(format!("skill {name}: {reason}"));
                    resolved.held_skills.insert(name);
                }
            }
        }
        resolved.watched.sort();
        resolved.watched.dedup();
        resolved
    }

    fn watch(&mut self, url: &str, git_ref: &str, sha: &str) {
        self.watched.extend(Watched::new(url, git_ref, sha));
    }
}

fn mcp_source(
    name: &str,
    url: &str,
    git_ref: &str,
    requires: &[String],
    build: &[String],
    sources: &Path,
) -> Result<McpSource, String> {
    let sha = git::resolve(url, git_ref).map_err(|e| format!("fonte inacessivel: {e:#}"))?;
    let missing = source::missing_requires(requires);
    if !missing.is_empty() {
        return Err(format!(
            "requires ausente no PATH: {}; build nao executado",
            missing.join(", ")
        ));
    }
    let dir = source::checkout_dir(sources, name, &sha);
    let job = (!source::is_built(&dir, build)).then(|| BuildJob {
        name: name.to_string(),
        repo: url.to_string(),
        git_ref: git_ref.to_string(),
        sha: sha.clone(),
        commands: build.to_vec(),
        dir: dir.clone(),
    });
    Ok(McpSource { sha, dir, job })
}

/// Git work already done in this run, so skills from the same `git` and `ref` (as a
/// `paths` entry gives) resolve the ref once and fetch the repo once.
#[derive(Default)]
struct Fetched {
    shas: BTreeMap<(String, String), Result<String, String>>,
    checkouts: BTreeMap<(String, String), PathBuf>,
}

impl Fetched {
    fn skill_source(
        &mut self,
        name: &str,
        url: &str,
        git_ref: &str,
        path: &str,
        sources: &Path,
    ) -> Result<SkillSource, String> {
        let sha = self.resolve(url, git_ref)?;
        let checkout = source::checkout_dir(sources, name, &sha);
        self.ensure_checkout(url, git_ref, &sha, &checkout)
            .map_err(|e| format!("checkout falhou: {e:#}"))?;
        let dir = checkout.join(path);
        if !dir.join("SKILL.md").is_file() {
            return Err(format!("'{path}' sem SKILL.md em {url} ({sha})"));
        }
        Ok(SkillSource { sha, dir })
    }

    fn resolve(&mut self, url: &str, git_ref: &str) -> Result<String, String> {
        let key = (url.to_string(), git_ref.to_string());
        self.shas
            .entry(key)
            .or_insert_with(|| {
                git::resolve(url, git_ref).map_err(|e| format!("fonte inacessivel: {e:#}"))
            })
            .clone()
    }

    /// Checks out `sha` into `dir`, from a checkout of the same commit made earlier in
    /// this run when there is one, else from `url`.
    fn ensure_checkout(
        &mut self,
        url: &str,
        git_ref: &str,
        sha: &str,
        dir: &Path,
    ) -> anyhow::Result<()> {
        let key = (url.to_string(), sha.to_string());
        match self.checkouts.get(&key) {
            Some(sibling) => {
                let sibling = sibling.to_string_lossy().to_string();
                source::ensure_checkout(&sibling, sha, sha, dir)?;
            }
            None => source::ensure_checkout(url, git_ref, sha, dir)?,
        }
        self.checkouts.insert(key, dir.to_path_buf());
        Ok(())
    }
}
