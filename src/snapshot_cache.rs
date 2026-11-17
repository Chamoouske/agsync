//! Checkouts of the config repo by url and commit. A commit never changes, so an
//! entry never goes stale; entries unused for [`MAX_AGE`] are removed.
//!
//! An entry is `<key>/tree` (the checked-out files, without `.git`) and `<key>/used`
//! (its mtime marks the last use). It is built in a temp dir beside it and renamed
//! into place, so an entry that has `used` is complete.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use anyhow::Context;

use crate::tmp::TempDir;

/// Entries not used for this long are removed on the next checkout.
pub const MAX_AGE: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// A hit marks the entry as used only when it was last marked longer ago than this.
const TOUCH_AFTER: Duration = Duration::from_secs(24 * 60 * 60);

const TREE: &str = "tree";
const USED: &str = "used";

/// Files of `url` at `sha`: the cached tree when there is one, else checked out once
/// (from `git_ref`, see [`crate::git::checkout`]) and kept. A hit marks the entry as used.
pub fn open(dir: &Path, url: &str, git_ref: &str, sha: &str) -> anyhow::Result<PathBuf> {
    let entry = entry(dir, url, sha);
    let now = SystemTime::now();
    if is_complete(&entry) {
        touch(&entry.join(USED), now);
        return Ok(entry.join(TREE));
    }
    fill(dir, url, git_ref, sha, &entry)?;
    evict(dir, now);
    if !is_complete(&entry) {
        anyhow::bail!("cache de snapshot incompleto em {}", entry.display());
    }
    Ok(entry.join(TREE))
}

fn entry(dir: &Path, url: &str, sha: &str) -> PathBuf {
    let key = crate::hash::hash_bytes(format!("{url}\n{sha}").as_bytes());
    dir.join(key)
}

fn is_complete(entry: &Path) -> bool {
    is_real_dir(entry) && is_real_dir(&entry.join(TREE)) && is_real_file(&entry.join(USED))
}

/// Checks out into a temp dir beside `entry`, then renames it into place. When another
/// process wins the rename, its entry is kept and the temp dir is dropped.
fn fill(dir: &Path, url: &str, git_ref: &str, sha: &str, entry: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir).with_context(|| format!("criar {}", dir.display()))?;
    let tmp = TempDir::new_in(dir, "tmp")?;
    let tree = tmp.path().join(TREE);
    crate::git::checkout(url, git_ref, sha, &tree)?;
    let git_dir = tree.join(".git");
    std::fs::remove_dir_all(&git_dir).with_context(|| format!("apagar {}", git_dir.display()))?;
    std::fs::write(tmp.path().join(USED), "")?;
    if is_complete(entry) {
        return Ok(());
    }
    remove(entry);
    if std::fs::rename(tmp.path(), entry).is_err() && !is_complete(entry) {
        anyhow::bail!("mover snapshot para {}", entry.display());
    }
    Ok(())
}

/// Removes whatever is at `path` without following a symlink. A directory symlink on
/// Windows is removed with `remove_dir`, which drops the link only.
fn remove(path: &Path) {
    if is_real_dir(path) {
        std::fs::remove_dir_all(path).ok();
        return;
    }
    if std::fs::remove_file(path).is_err() {
        std::fs::remove_dir(path).ok();
    }
}

fn touch(used: &Path, now: SystemTime) {
    let stale = age(used, now).is_some_and(|age| age > TOUCH_AFTER);
    if !stale {
        return;
    }
    if let Ok(file) = std::fs::File::options().write(true).open(used) {
        file.set_modified(now).ok();
    }
}

/// Removes entries whose `used` is older than [`MAX_AGE`], and temp dirs left by a
/// crash once they are that old. `used` goes first, so a crash mid-removal leaves an
/// entry that [`open`] checks out again. Symlinks are never followed.
fn evict(dir: &Path, now: SystemTime) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !is_real_dir(&path) {
            continue;
        }
        let used = path.join(USED);
        let last_use = age(&used, now).or_else(|| dir_age(&path, now));
        let old = last_use.is_some_and(|age| age > MAX_AGE);
        if old {
            std::fs::remove_file(&used).ok();
            std::fs::remove_dir_all(&path).ok();
        }
    }
}

/// Time since `path` was last modified; `None` for anything but a regular file.
fn age(path: &Path, now: SystemTime) -> Option<Duration> {
    if !is_real_file(path) {
        return None;
    }
    let modified = std::fs::symlink_metadata(path).ok()?.modified().ok()?;
    now.duration_since(modified).ok()
}

fn dir_age(path: &Path, now: SystemTime) -> Option<Duration> {
    let modified = std::fs::symlink_metadata(path).ok()?.modified().ok()?;
    now.duration_since(modified).ok()
}

fn is_real_dir(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_dir())
}

fn is_real_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::git;

    fn run(dir: &Path, args: &[&str]) -> String {
        let mut all = vec![
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ];
        all.extend_from_slice(args);
        git(&all, Some(dir)).unwrap()
    }

    struct Remote {
        root: TempDir,
        url: String,
        first: String,
        head: String,
    }

    impl Remote {
        fn new() -> Remote {
            let root = TempDir::new("agsync-test").unwrap();
            let r = root.path().join("remote");
            std::fs::create_dir_all(&r).unwrap();
            run(&r, &["init", "-q", "-b", "main"]);
            std::fs::write(r.join("agsync.yaml"), "version: 1\n").unwrap();
            run(&r, &["add", "."]);
            run(&r, &["commit", "-q", "-m", "one"]);
            let first = run(&r, &["rev-parse", "HEAD"]).trim().to_string();
            std::fs::write(r.join("agsync.yaml"), "version: 1\nprofiles: []\n").unwrap();
            run(&r, &["commit", "-q", "-am", "two"]);
            let head = run(&r, &["rev-parse", "HEAD"]).trim().to_string();
            let url = r.to_string_lossy().to_string();
            Remote {
                root,
                url,
                first,
                head,
            }
        }

        fn cache(&self) -> PathBuf {
            self.root.path().join("cache")
        }

        fn delete(&self) {
            std::fs::remove_dir_all(self.root.path().join("remote")).unwrap();
        }
    }

    fn yaml(tree: &Path) -> String {
        std::fs::read_to_string(tree.join("agsync.yaml")).unwrap()
    }

    fn set_mtime(path: &Path, time: SystemTime) {
        let file = std::fs::File::options().write(true).open(path).unwrap();
        file.set_modified(time).unwrap();
    }

    fn mtime(path: &Path) -> SystemTime {
        std::fs::metadata(path).unwrap().modified().unwrap()
    }

    #[test]
    fn checks_out_once_per_url_and_sha() {
        let remote = Remote::new();
        let cache = remote.cache();
        let tree = open(&cache, &remote.url, &remote.first, &remote.first).unwrap();
        assert_eq!(yaml(&tree), "version: 1\n");
        assert!(!tree.join(".git").exists());
        let head = open(&cache, &remote.url, "main", &remote.head).unwrap();
        assert_eq!(yaml(&head), "version: 1\nprofiles: []\n");

        remote.delete();
        let again = open(&cache, &remote.url, &remote.first, &remote.first).unwrap();
        assert_eq!(again, tree);
        assert_eq!(yaml(&again), "version: 1\n");
        let other = format!("{}-other", remote.url);
        assert!(open(&cache, &other, &remote.first, &remote.first).is_err());
        let unknown = "b".repeat(40);
        assert!(open(&cache, &remote.url, &unknown, &unknown).is_err());
    }

    #[test]
    fn partial_entry_is_checked_out_again() {
        let remote = Remote::new();
        let cache = remote.cache();
        let tree = open(&cache, &remote.url, "main", &remote.head).unwrap();
        let key = entry(&cache, &remote.url, &remote.head);
        std::fs::remove_file(key.join(USED)).unwrap();
        std::fs::write(tree.join("agsync.yaml"), "version: [").unwrap();
        let tree = open(&cache, &remote.url, "main", &remote.head).unwrap();
        assert_eq!(yaml(&tree), "version: 1\nprofiles: []\n");
        assert!(key.join(USED).is_file());
    }

    #[test]
    fn evicts_only_old_entries() {
        let remote = Remote::new();
        let cache = remote.cache();
        open(&cache, &remote.url, &remote.first, &remote.first).unwrap();
        open(&cache, &remote.url, "main", &remote.head).unwrap();
        let old = entry(&cache, &remote.url, &remote.first);
        let new = entry(&cache, &remote.url, &remote.head);
        let later = SystemTime::now() + MAX_AGE + Duration::from_secs(60);
        set_mtime(&new.join(USED), later);
        evict(&cache, later);
        assert!(!old.exists());
        assert!(new.join(TREE).join("agsync.yaml").is_file());
    }

    #[test]
    fn hit_marks_use_at_most_once_a_day() {
        let remote = Remote::new();
        let cache = remote.cache();
        open(&cache, &remote.url, "main", &remote.head).unwrap();
        let used = entry(&cache, &remote.url, &remote.head).join(USED);
        let hour_ago = SystemTime::now() - Duration::from_secs(60 * 60);
        set_mtime(&used, hour_ago);
        let before = mtime(&used);
        open(&cache, &remote.url, "main", &remote.head).unwrap();
        assert_eq!(mtime(&used), before);
        set_mtime(&used, SystemTime::now() - 2 * TOUCH_AFTER);
        open(&cache, &remote.url, "main", &remote.head).unwrap();
        assert!(mtime(&used) > hour_ago);
    }

    #[cfg(unix)]
    fn symlink_dir(original: &Path, link: &Path) -> bool {
        std::os::unix::fs::symlink(original, link).is_ok()
    }

    #[cfg(windows)]
    fn symlink_dir(original: &Path, link: &Path) -> bool {
        std::os::windows::fs::symlink_dir(original, link).is_ok()
    }

    #[test]
    fn symlinks_are_never_followed() {
        let remote = Remote::new();
        let cache = remote.cache();
        let outside = remote.root.path().join("outside");
        std::fs::create_dir_all(outside.join(TREE)).unwrap();
        std::fs::write(outside.join(TREE).join("agsync.yaml"), "planted").unwrap();
        std::fs::write(outside.join(USED), "").unwrap();
        let old = SystemTime::now() - 2 * MAX_AGE;
        set_mtime(&outside.join(USED), old);
        std::fs::create_dir_all(&cache).unwrap();
        let link = entry(&cache, &remote.url, &remote.head);
        if !symlink_dir(&outside, &link) {
            eprintln!("SKIP symlinks_are_never_followed: sem permissao para symlink");
            return;
        }

        let tree = open(&cache, &remote.url, "main", &remote.head).unwrap();
        assert_eq!(yaml(&tree), "version: 1\nprofiles: []\n");
        assert!(mtime(&outside.join(USED)) < SystemTime::now() - MAX_AGE);

        let other = entry(&cache, &remote.url, &remote.first);
        assert!(symlink_dir(&outside, &other));
        evict(&cache, SystemTime::now());
        assert!(std::fs::symlink_metadata(&other).is_ok());
        assert!(outside.join(TREE).join("agsync.yaml").is_file());
        assert!(outside.join(USED).is_file());
    }
}
