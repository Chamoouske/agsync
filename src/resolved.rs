//! Git sources of the active items, resolved once per run.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

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

/// Sources by item name, plus the items held back and why.
#[derive(Debug, Default)]
pub struct Resolved {
    pub mcps: BTreeMap<String, McpSource>,
    pub skills: BTreeMap<String, SkillSource>,
    pub held_mcps: BTreeSet<String>,
    pub held_skills: BTreeSet<String>,
    pub skipped: Vec<String>,
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
                    resolved.mcps.insert(name.clone(), found);
                }
                Err(reason) => {
                    resolved.held_mcps.insert(name.clone());
                    resolved.skipped.push(format!("mcp {name}: {reason}"));
                }
            }
        }
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
            match skill_source(&name, url, git_ref, path, &sources) {
                Ok(found) => {
                    resolved.skills.insert(name, found);
                }
                Err(reason) => {
                    resolved.skipped.push(format!("skill {name}: {reason}"));
                    resolved.held_skills.insert(name);
                }
            }
        }
        resolved
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

fn skill_source(
    name: &str,
    url: &str,
    git_ref: &str,
    path: &str,
    sources: &Path,
) -> Result<SkillSource, String> {
    let sha = git::resolve(url, git_ref).map_err(|e| format!("fonte inacessivel: {e:#}"))?;
    let checkout = source::checkout_dir(sources, name, &sha);
    source::ensure_checkout(url, git_ref, &sha, &checkout)
        .map_err(|e| format!("checkout falhou: {e:#}"))?;
    let dir = checkout.join(path);
    if !dir.join("SKILL.md").is_file() {
        return Err(format!("'{path}' sem SKILL.md em {url} ({sha})"));
    }
    Ok(SkillSource { sha, dir })
}
