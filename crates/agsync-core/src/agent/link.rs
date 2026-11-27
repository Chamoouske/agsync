//! Skill folders: content-addressed cache and links from agent skills dirs into it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;

use crate::hash::hash_dir;
use crate::state::LinkMode;

/// Comma list of link kinds (`symlink`, `junction`) that fail as if not allowed here:
/// a symlink without Windows Developer Mode, a junction to another volume (tests).
pub const TEST_FAIL_VAR: &str = "AGSYNC_TEST_LINK_FAIL";

/// Skill folders (or links to folders) in an agent skills dir: name to content hash.
pub fn installed(skills_dir: &Path) -> anyhow::Result<BTreeMap<String, String>> {
    let mut found = BTreeMap::new();
    let entries = match std::fs::read_dir(skills_dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(found),
        Err(e) => return Err(e).with_context(|| format!("ler {}", skills_dir.display())),
    };
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            let hash = hash_dir(&path).unwrap_or_else(|_| "ilegivel".to_string());
            found.insert(name, hash);
        } else if std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_symlink()) {
            found.insert(name, "link-quebrado".to_string());
        }
    }
    Ok(found)
}

/// Ensures `<cache_root>/<name>/<hash>/` holds a copy of `src`; returns its path.
/// A cache folder whose content no longer matches `hash` is moved to `backups`.
pub fn ensure_cache(
    src: &Path,
    cache_root: &Path,
    name: &str,
    hash: &str,
    backups: &Path,
) -> anyhow::Result<PathBuf> {
    let dir = cache_root.join(name).join(hash);
    if dir.is_dir() {
        if hash_dir(&dir).ok().as_deref() == Some(hash) {
            return Ok(dir);
        }
        move_to_backup(&dir, backups, &format!("cache-{name}"))?;
    }
    let tmp = cache_root.join(name).join(format!("{hash}.tmp"));
    if tmp.exists() {
        std::fs::remove_dir_all(&tmp).with_context(|| format!("limpar {}", tmp.display()))?;
    }
    copy_dir(src, &tmp)?;
    std::fs::rename(&tmp, &dir).with_context(|| format!("renomear para {}", dir.display()))?;
    Ok(dir)
}

/// Points `target` at `cached`: directory symlink, then junction (Windows), then copy.
/// Whatever was at `target` before is cleared first (see [`clear`]).
pub fn install(cached: &Path, target: &Path, backups: &Path) -> anyhow::Result<LinkMode> {
    clear(target, backups)?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if symlink_dir(cached, target).is_ok() {
        return Ok(LinkMode::Symlink);
    }
    if cfg!(windows) && junction(cached, target).is_ok() {
        return Ok(LinkMode::Junction);
    }
    copy_dir(cached, target)?;
    Ok(LinkMode::Copy)
}

/// Removes `target`: links are deleted, real folders are moved to `backups`.
pub fn clear(target: &Path, backups: &Path) -> anyhow::Result<()> {
    let Ok(meta) = std::fs::symlink_metadata(target) else {
        return Ok(());
    };
    if meta.is_symlink() {
        return std::fs::remove_file(target)
            .or_else(|_| std::fs::remove_dir(target))
            .with_context(|| format!("remover link {}", target.display()));
    }
    if meta.is_dir() {
        let name = target.file_name().unwrap_or_default().to_string_lossy();
        return move_to_backup(target, backups, &format!("skill-{name}"));
    }
    std::fs::remove_file(target).with_context(|| format!("remover {}", target.display()))
}

/// Recursive copy of a folder.
pub fn copy_dir(src: &Path, dest: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dest).with_context(|| format!("criar {}", dest.display()))?;
    let entries = std::fs::read_dir(src).with_context(|| format!("ler {}", src.display()))?;
    for entry in entries {
        let entry = entry?;
        let from = entry.path();
        let to = dest.join(entry.file_name());
        if from.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            std::fs::copy(&from, &to).with_context(|| format!("copiar {}", from.display()))?;
        }
    }
    Ok(())
}

fn move_to_backup(dir: &Path, backups: &Path, label: &str) -> anyhow::Result<()> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let dest = backups.join(format!("{label}-{millis}"));
    std::fs::create_dir_all(backups)?;
    if std::fs::rename(dir, &dest).is_ok() {
        return Ok(());
    }
    copy_dir(dir, &dest)?;
    std::fs::remove_dir_all(dir).with_context(|| format!("remover {}", dir.display()))
}

fn forced_to_fail(kind: &str) -> bool {
    let kinds = std::env::var(TEST_FAIL_VAR).unwrap_or_default();
    kinds.split(',').any(|k| k.trim() == kind)
}

/// Directory symlink. Forced to fail with `ERROR_PRIVILEGE_NOT_HELD`, the error of
/// Windows without Developer Mode.
#[cfg(windows)]
fn symlink_dir(original: &Path, link: &Path) -> std::io::Result<()> {
    if forced_to_fail("symlink") {
        return Err(std::io::Error::from_raw_os_error(1314));
    }
    std::os::windows::fs::symlink_dir(original, link)
}

/// Directory symlink. Forced to fail with `EPERM`.
#[cfg(not(windows))]
fn symlink_dir(original: &Path, link: &Path) -> std::io::Result<()> {
    if forced_to_fail("symlink") {
        return Err(std::io::Error::from_raw_os_error(1));
    }
    std::os::unix::fs::symlink(original, link)
}

fn junction(original: &Path, link: &Path) -> anyhow::Result<()> {
    if forced_to_fail("junction") {
        anyhow::bail!("mklink /J falhou ({TEST_FAIL_VAR})");
    }
    let status = std::process::Command::new("cmd")
        .arg("/C")
        .arg("mklink")
        .arg("/J")
        .arg(link)
        .arg(original)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()?;
    if !status.success() || !link.is_dir() {
        anyhow::bail!("mklink /J falhou");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmp::TempDir;

    fn skill(root: &Path, text: &str) -> PathBuf {
        let src = root.join("src").join("hello");
        std::fs::create_dir_all(src.join("refs")).unwrap();
        std::fs::write(src.join("SKILL.md"), text).unwrap();
        std::fs::write(src.join("refs").join("a.md"), "ref").unwrap();
        src
    }

    #[test]
    fn cache_then_link_then_clear() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let root = tmp.path();
        let backups = root.join("backups");
        let src = skill(root, "v1");
        let hash = hash_dir(&src).unwrap();

        let cached = ensure_cache(&src, &root.join("cache"), "hello", &hash, &backups).unwrap();
        assert_eq!(cached, root.join("cache").join("hello").join(&hash));
        assert_eq!(hash_dir(&cached).unwrap(), hash);

        let target = root.join("agent").join("skills").join("hello");
        let mode = install(&cached, &target, &backups).unwrap();
        eprintln!("skill link mode on this machine: {mode:?}");
        assert_eq!(hash_dir(&target).unwrap(), hash);

        install(&cached, &target, &backups).unwrap();
        assert_eq!(hash_dir(&target).unwrap(), hash);

        clear(&target, &backups).unwrap();
        assert!(std::fs::symlink_metadata(&target).is_err());
        assert!(cached.join("SKILL.md").is_file());
    }

    #[test]
    fn real_folder_at_target_goes_to_backup() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let root = tmp.path();
        let backups = root.join("backups");
        let target = root.join("skills").join("hello");
        std::fs::create_dir_all(&target).unwrap();
        std::fs::write(target.join("SKILL.md"), "manual").unwrap();

        clear(&target, &backups).unwrap();
        assert!(!target.exists());
        let saved: Vec<_> = std::fs::read_dir(&backups).unwrap().collect();
        assert_eq!(saved.len(), 1);
    }

    #[test]
    fn tampered_cache_is_rebuilt() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let root = tmp.path();
        let backups = root.join("backups");
        let src = skill(root, "v1");
        let hash = hash_dir(&src).unwrap();
        let cache = root.join("cache");
        let cached = ensure_cache(&src, &cache, "hello", &hash, &backups).unwrap();
        std::fs::write(cached.join("SKILL.md"), "edited").unwrap();

        let again = ensure_cache(&src, &cache, "hello", &hash, &backups).unwrap();
        assert_eq!(hash_dir(&again).unwrap(), hash);
        assert!(backups.is_dir());
    }

    #[cfg(windows)]
    #[test]
    fn junction_fallback_works_and_clears() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let src = skill(tmp.path(), "v1");
        let link = tmp.path().join("junction-hello");
        junction(&src, &link).unwrap();
        assert!(std::fs::symlink_metadata(&link).unwrap().is_symlink());
        assert_eq!(hash_dir(&link).unwrap(), hash_dir(&src).unwrap());
        clear(&link, &tmp.path().join("backups")).unwrap();
        assert!(std::fs::symlink_metadata(&link).is_err());
        assert!(src.join("SKILL.md").is_file());
    }

    #[test]
    fn copy_mode_is_a_full_copy() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let src = skill(tmp.path(), "v1");
        let dest = tmp.path().join("copy");
        copy_dir(&src, &dest).unwrap();
        assert_eq!(hash_dir(&dest).unwrap(), hash_dir(&src).unwrap());
    }
}
