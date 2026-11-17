//! Text of imported `agsync.yaml` files by url and commit, saved once they parsed
//! and their skill folders were found. A commit never changes, so an entry never
//! goes stale; entries unused for [`MAX_AGE`] are removed.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::manifest::Manifest;

/// Entries not read for this long are removed on the next save.
pub const MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// A hit marks the entry as used only when it was last marked longer ago than this.
const TOUCH_AFTER: Duration = Duration::from_secs(24 * 60 * 60);

/// Manifest saved for `url` at `sha`, or `None` when missing or unreadable.
/// A hit marks the entry as used.
pub fn load(dir: &Path, url: &str, sha: &str) -> Option<Manifest> {
    let path = entry(dir, url, sha);
    let text = std::fs::read_to_string(&path).ok()?;
    let manifest = Manifest::parse(&text).ok()?;
    touch(&path, SystemTime::now());
    Some(manifest)
}

/// Saves the checked `text` for `url` at `sha`, then removes old entries.
pub fn save(dir: &Path, url: &str, sha: &str, text: &str) -> anyhow::Result<()> {
    crate::atomic::write_atomic(&entry(dir, url, sha), text.as_bytes())?;
    evict(dir, SystemTime::now());
    Ok(())
}

fn entry(dir: &Path, url: &str, sha: &str) -> PathBuf {
    let key = crate::hash::hash_bytes(format!("{url}\n{sha}").as_bytes());
    dir.join(format!("{key}.yaml"))
}

fn touch(path: &Path, now: SystemTime) {
    let stale = age(path, now).is_some_and(|age| age > TOUCH_AFTER);
    if !stale {
        return;
    }
    if let Ok(file) = std::fs::File::options().write(true).open(path) {
        file.set_modified(now).ok();
    }
}

/// Time since `path` was last modified.
fn age(path: &Path, now: SystemTime) -> Option<Duration> {
    let modified = std::fs::metadata(path).and_then(|m| m.modified()).ok()?;
    now.duration_since(modified).ok()
}

fn evict(dir: &Path, now: SystemTime) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let old = age(&entry.path(), now).is_some_and(|age| age > MAX_AGE);
        if old {
            std::fs::remove_file(entry.path()).ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmp::TempDir;

    const YAML: &str = "version: 1\nmcps:\n  a:\n    command: node\n";

    fn set_mtime(path: &Path, time: SystemTime) {
        let file = std::fs::File::options().write(true).open(path).unwrap();
        file.set_modified(time).unwrap();
    }

    fn mtime(path: &Path) -> SystemTime {
        std::fs::metadata(path).unwrap().modified().unwrap()
    }

    #[test]
    fn saves_and_loads_by_url_and_sha() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let dir = tmp.path();
        assert!(load(dir, "u", "s1").is_none());
        save(dir, "u", "s1", YAML).unwrap();
        assert!(load(dir, "u", "s1").unwrap().mcps.contains_key("a"));
        assert!(load(dir, "u", "s2").is_none());
        assert!(load(dir, "v", "s1").is_none());
    }

    #[test]
    fn corrupt_entry_is_a_miss() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let dir = tmp.path();
        save(dir, "u", "s", YAML).unwrap();
        std::fs::write(entry(dir, "u", "s"), "version: [").unwrap();
        assert!(load(dir, "u", "s").is_none());
    }

    #[test]
    fn evicts_only_old_entries() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let dir = tmp.path();
        save(dir, "u", "old", YAML).unwrap();
        save(dir, "u", "new", YAML).unwrap();
        let later = SystemTime::now() + MAX_AGE + Duration::from_secs(60);
        set_mtime(&entry(dir, "u", "new"), later);
        evict(dir, later);
        assert!(!entry(dir, "u", "old").exists());
        assert!(entry(dir, "u", "new").exists());
    }

    #[test]
    fn hit_marks_use_at_most_once_a_day() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let dir = tmp.path();
        let path = entry(dir, "u", "s");
        save(dir, "u", "s", YAML).unwrap();
        let hour_ago = SystemTime::now() - Duration::from_secs(60 * 60);
        set_mtime(&path, hour_ago);
        let before = mtime(&path);
        assert!(load(dir, "u", "s").is_some());
        assert_eq!(mtime(&path), before);
        set_mtime(&path, SystemTime::now() - 2 * TOUCH_AFTER);
        assert!(load(dir, "u", "s").is_some());
        assert!(mtime(&path) > hour_ago);
    }
}
