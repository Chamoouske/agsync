//! `<data>/watch.json`: the config commit planned last and the moving git sources it
//! used, so `check` can stop after `ls-remote` when nothing moved.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::git;
use crate::resolved::Watched;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Watch {
    pub commit: String,
    #[serde(default)]
    pub sources: Vec<Watched>,
}

impl Watch {
    /// A missing or unreadable file means nothing was planned yet.
    pub fn load(path: &Path) -> Option<Watch> {
        let text = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let text = serde_json::to_string_pretty(self)?;
        crate::atomic::write_atomic(path, text.as_bytes())
    }

    /// True when `remote` is the commit planned last and no source moved since. Asks
    /// `ls-remote` once per source; a source out of reach counts as not moved.
    pub fn unchanged(&self, remote: &str) -> bool {
        self.commit == remote && self.sources.iter().all(still_at_sha)
    }
}

fn still_at_sha(source: &Watched) -> bool {
    git::resolve(&source.url, &source.git_ref).map_or(true, |sha| sha == source.sha)
}
