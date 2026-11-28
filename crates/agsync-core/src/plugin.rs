//! One plugin from a git marketplace, pinned to a ref.

use std::path::Path;

use crate::hash::hash_bytes;
use crate::plan::Want;
use crate::state::Tracked;

/// Plugin as written in `agsync.yaml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginSpec {
    /// Name declared by the plugin inside the marketplace.
    pub name: String,
    /// Marketplace git URL, GitHub shorthand or local folder.
    pub git: String,
    pub git_ref: String,
}

impl PluginSpec {
    pub fn new(
        name: impl Into<String>,
        git: impl Into<String>,
        git_ref: impl Into<String>,
    ) -> Self {
        PluginSpec {
            name: name.into(),
            git: git.into(),
            git_ref: git_ref.into(),
        }
    }

    /// Identity in `state.json`: the source and ref only, never a credential.
    pub fn hash(&self) -> String {
        let text = format!("{}\0{}\0{}", self.name, self.git, self.git_ref);
        hash_bytes(text.as_bytes())
    }

    pub fn want(&self) -> Want {
        let hash = self.hash();
        Want {
            key: hash.clone(),
            write: hash,
        }
    }

    /// State entry, with the id the agent uses to uninstall the plugin later.
    pub fn tracked(&self, plugin: String) -> Tracked {
        Tracked {
            hash: self.hash(),
            mode: None,
            sha: None,
            plugin: Some(plugin),
        }
    }

    /// Source shown by `diff`: `git#ref`, or the bare folder for a local marketplace.
    pub fn describe(&self) -> String {
        if self.is_local() {
            self.git.clone()
        } else {
            format!("{}#{}", self.git, self.git_ref)
        }
    }

    /// Package spec OpenCode writes to its config.
    pub fn opencode_package(&self) -> String {
        let git = &self.git;
        let is_shorthand = !git.contains("://")
            && !git.starts_with("git@")
            && !self.is_local()
            && !git.starts_with("github:");
        if git.starts_with("github:") {
            format!("{git}#{}", self.git_ref)
        } else if is_shorthand {
            format!("github:{git}#{}", self.git_ref)
        } else {
            format!("git+{git}#{}", self.git_ref)
        }
    }

    /// What `git clone` takes: `owner/repo` and `github:owner/repo` become a GitHub
    /// https URL; URLs and local folders stay as written.
    pub fn clone_url(&self) -> String {
        let git = &self.git;
        if let Some(repo) = git.strip_prefix("github:") {
            return format!("https://github.com/{repo}");
        }
        let is_shorthand = !git.contains("://") && !git.starts_with("git@") && !self.is_local();
        if is_shorthand {
            return format!("https://github.com/{git}");
        }
        git.clone()
    }

    /// True when the marketplace is a folder on this machine. Claude Code reads it in
    /// place and does not accept a ref for it.
    pub fn is_local(&self) -> bool {
        is_local_marketplace(&self.git)
    }
}

/// True for `./`, `../`, `/`, `\`, `file:` and drive-letter paths.
pub fn is_local_marketplace(git: &str) -> bool {
    let bytes = git.as_bytes();
    git.starts_with("./")
        || git.starts_with("../")
        || git.starts_with('/')
        || git.starts_with('\\')
        || git.starts_with("file:")
        || (bytes.len() > 2
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && (bytes[2] == b'\\' || bytes[2] == b'/'))
        || Path::new(git).is_absolute()
}

/// Key to compare two spellings of the same repository.
///
/// `owner/repo`, `https://github.com/owner/repo.git` and `git@github.com:owner/repo.git`
/// all become `owner/repo`; other hosts keep the host. Local folders are compared by
/// their canonical path when they exist.
pub fn repo_key(git: &str) -> String {
    let trimmed = git.trim().trim_end_matches('/');
    let without_ext = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    if is_local_marketplace(without_ext) {
        return local_key(without_ext);
    }
    if let Some(rest) = without_ext.strip_prefix("git@")
        && let Some((host, path)) = rest.split_once(':')
    {
        return with_host(host, path.trim_start_matches('/'));
    }
    if let Some((_, rest)) = without_ext.split_once("://")
        && let Some((host, path)) = rest.split_once('/')
    {
        return with_host(host, path);
    }
    without_ext.to_lowercase()
}

/// GitHub keeps only `owner/repo`; other hosts keep the host, so two GitLab servers
/// with the same group and repo do not compare equal.
fn with_host(host: &str, path: &str) -> String {
    let host = host.to_lowercase();
    let host = host.split('@').next_back().unwrap_or(&host);
    if host == "github.com" || host == "www.github.com" {
        return path.to_lowercase();
    }
    format!("{host}/{}", path.to_lowercase())
}

fn local_key(path: &str) -> String {
    let key = match std::fs::canonicalize(path) {
        Ok(path) => path.to_string_lossy().into_owned(),
        Err(_) => path.replace('\\', "/"),
    };
    if cfg!(windows) {
        key.to_lowercase()
    } else {
        key
    }
}

/// True when both describe the same place, using [`repo_key`].
pub fn same_repo(a: &str, b: &str) -> bool {
    repo_key(a) == repo_key(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_stable_and_has_no_secret() {
        let spec = PluginSpec::new("p1", "https://git.example.com/t/p.git", "v1");
        assert_eq!(spec.hash(), spec.hash());
        assert_eq!(spec.want().key, spec.hash());
        assert_eq!(spec.want().write, spec.hash());
        let other = PluginSpec::new("p1", "https://git.example.com/t/p.git", "v2");
        assert_ne!(spec.hash(), other.hash());
        assert!(!spec.describe().contains("token"));
    }

    #[test]
    fn local_marketplace_has_no_ref_in_description() {
        let spec = PluginSpec::new("p1", "./market", "v1");
        assert!(spec.is_local());
        assert_eq!(spec.describe(), "./market");
        assert!(PluginSpec::new("p1", "/srv/market", "v1").is_local());
        assert!(PluginSpec::new("p1", r"C:\mkt", "v1").is_local());
        assert!(!PluginSpec::new("p1", "owner/repo", "v1").is_local());
    }

    #[test]
    fn opencode_package_uses_git_plus_and_ref() {
        let spec = PluginSpec::new("p1", "https://git.example.com/t/p.git", "v1");
        assert_eq!(
            spec.opencode_package(),
            "git+https://git.example.com/t/p.git#v1"
        );
        let github = PluginSpec::new("p1", "github:owner/repo", "main");
        assert_eq!(github.opencode_package(), "github:owner/repo#main");
        let shorthand = PluginSpec::new("p1", "owner/repo", "main");
        assert_eq!(shorthand.opencode_package(), "github:owner/repo#main");
    }

    #[test]
    fn clone_url_expands_github_shorthand_only() {
        let url = |git: &str| PluginSpec::new("p", git, "v1").clone_url();
        assert_eq!(url("owner/repo"), "https://github.com/owner/repo");
        assert_eq!(url("github:owner/repo"), "https://github.com/owner/repo");
        assert_eq!(
            url("https://x.example/t/p.git"),
            "https://x.example/t/p.git"
        );
        assert_eq!(url("git@x.example:t/p.git"), "git@x.example:t/p.git");
        assert_eq!(url("./market"), "./market");
    }

    #[test]
    fn repo_keys_compare_urls_and_shorthand() {
        for git in [
            "owner/repo",
            "https://github.com/owner/repo.git",
            "https://github.com/owner/repo",
            "git@github.com:owner/repo.git",
        ] {
            assert_eq!(repo_key(git), "owner/repo", "{git}");
        }
        assert_eq!(
            repo_key("https://gitlab.example.com/group/repo.git"),
            "gitlab.example.com/group/repo"
        );
        assert!(same_repo(
            "https://gitlab.example.com/group/repo.git",
            "https://gitlab.example.com/group/repo"
        ));
        assert!(same_repo(
            "git@gitlab.example.com:group/repo.git",
            "https://gitlab.example.com/group/repo"
        ));
        assert!(!same_repo("owner/a", "owner/b"));
    }
}
