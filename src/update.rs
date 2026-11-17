//! New agsync releases: `agsync check` reads the release tags, records a newer version
//! in `update.json` and notifies once per version. `agsync hook` prints it, offline.

use std::fmt;
use std::path::Path;

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::notify;
use crate::paths::Dirs;

/// Git repo whose `vX.Y.Z` tags are the releases.
const RELEASES_URL: &str = "https://github.com/Chamoouske/agsync";

/// Overrides [`RELEASES_URL`] (tests use a local repo).
pub const RELEASES_VAR: &str = "AGSYNC_RELEASES_URL";

/// A `major.minor.patch` version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u64, pub u64, pub u64);

impl Version {
    /// `X.Y.Z` with numbers only; anything else is `None`.
    pub fn parse(text: &str) -> Option<Version> {
        let mut parts = text.split('.').map(|p| p.parse::<u64>().ok());
        let version = Version(parts.next()??, parts.next()??, parts.next()??);
        if parts.next().is_some() {
            return None;
        }
        Some(version)
    }

    /// Version of this binary, from `Cargo.toml`.
    pub fn current() -> Version {
        Version::parse(env!("CARGO_PKG_VERSION")).unwrap_or(Version(0, 0, 0))
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// Content of `update.json`.
#[derive(Serialize, Deserialize)]
struct Saved {
    version: String,
}

/// Highest `vX.Y.Z` tag of the release repo.
pub fn latest() -> anyhow::Result<Version> {
    let url = std::env::var(RELEASES_VAR).unwrap_or_else(|_| RELEASES_URL.to_string());
    let args = [
        "-c",
        "protocol.ext.allow=never",
        "ls-remote",
        "--tags",
        "--refs",
        "--end-of-options",
        &url,
    ];
    let out = crate::git::git(&args, None)?;
    highest_tag(&out).with_context(|| format!("nenhuma tag vX.Y.Z em {url}"))
}

/// Highest `refs/tags/vX.Y.Z` in `git ls-remote` output; other tags are ignored.
fn highest_tag(ls_remote: &str) -> Option<Version> {
    ls_remote
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .filter_map(|(_, name)| name.trim().strip_prefix("refs/tags/v"))
        .filter_map(Version::parse)
        .max()
}

/// Run by `agsync check`: records or clears `update.json` and notifies a new version
/// once. Never fails; the returned text goes to `check.log`.
pub fn refresh(dirs: &Dirs) -> String {
    let latest = match latest() {
        Ok(version) => version,
        Err(e) => return format!("versao nova do agsync: consulta falhou: {e:#}"),
    };
    record(dirs, latest).unwrap_or_else(|e| format!("versao nova do agsync: {e:#}"))
}

fn record(dirs: &Dirs, latest: Version) -> anyhow::Result<String> {
    let current = Version::current();
    if latest <= current {
        clear(dirs)?;
        return Ok(format!("agsync {current} e a ultima versao"));
    }
    let file = dirs.update_file();
    if saved(&file) == Some(latest) {
        return Ok(format!("agsync {latest} disponivel (ja avisado)"));
    }
    let text = serde_json::to_string_pretty(&Saved {
        version: latest.to_string(),
    })?;
    crate::atomic::write_atomic(&file, text.as_bytes())?;
    let message = line(latest);
    match notify::for_this_machine().notify("agsync", &message) {
        Ok(()) => Ok(format!("{message} (notificado)")),
        Err(e) => Ok(format!("{message} (notificacao nao mostrada: {e:#})")),
    }
}

/// The version in `update.json` while it is newer than this binary.
pub fn recorded(dirs: &Dirs) -> Option<Version> {
    saved(&dirs.update_file()).filter(|v| *v > Version::current())
}

fn saved(file: &Path) -> Option<Version> {
    let text = std::fs::read_to_string(file).ok()?;
    let saved: Saved = serde_json::from_str(&text).ok()?;
    Version::parse(&saved.version)
}

/// Deletes `update.json`; a missing file is fine.
pub fn clear(dirs: &Dirs) -> anyhow::Result<()> {
    let file = dirs.update_file();
    match std::fs::remove_file(&file) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(e).with_context(|| format!("apagar {}", file.display()))
        }
        _ => Ok(()),
    }
}

/// The line printed by `agsync hook` and sent as notification.
pub fn line(newer: Version) -> String {
    format!(
        "agsync {newer} disponivel (atual {}): rode `agsync update`",
        Version::current()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_three_numbers() {
        assert_eq!(Version::parse("0.11.1"), Some(Version(0, 11, 1)));
        assert_eq!(Version::parse("1.2"), None);
        assert_eq!(Version::parse("1.2.3.4"), None);
        assert_eq!(Version::parse("1.2.3-rc1"), None);
        assert_eq!(Version::parse("a.b.c"), None);
        assert_eq!(Version::parse(""), None);
    }

    #[test]
    fn compares_numerically() {
        assert!(Version(0, 10, 0) > Version(0, 9, 9));
        assert!(Version(1, 0, 0) > Version(0, 99, 99));
        assert_eq!(Version(0, 2, 0).to_string(), "0.2.0");
        assert_eq!(Version::current().to_string(), env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn highest_semver_tag_wins() {
        let out = "aaa\trefs/tags/v0.9.0\n\
                   bbb\trefs/tags/v0.10.0\n\
                   ccc\trefs/tags/v0.10.0-rc1\n\
                   ddd\trefs/tags/latest\n\
                   eee\trefs/tags/v2\n\
                   fff\trefs/tags/0.99.0\n";
        assert_eq!(highest_tag(out), Some(Version(0, 10, 0)));
        assert_eq!(highest_tag("abc\trefs/tags/nightly\n"), None);
        assert_eq!(highest_tag(""), None);
    }
}
