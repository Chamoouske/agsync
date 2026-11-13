//! SHA-256 content hashes for written entries.

use std::path::Path;

use anyhow::Context;
use sha2::{Digest, Sha256};

/// Hex SHA-256 of `bytes`.
pub fn hash_bytes(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// Hash of a folder: relative paths and contents of every file, in sorted order.
pub fn hash_dir(dir: &Path) -> anyhow::Result<String> {
    let mut files = Vec::new();
    collect_files(dir, "", &mut files)?;
    files.sort();
    let mut hasher = Sha256::new();
    for rel in files {
        let bytes = std::fs::read(dir.join(&rel)).with_context(|| format!("ler {rel}"))?;
        hasher.update(rel.as_bytes());
        hasher.update([0]);
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
    }
    Ok(hex(&hasher.finalize()))
}

fn collect_files(dir: &Path, prefix: &str, out: &mut Vec<String>) -> anyhow::Result<()> {
    let entries = std::fs::read_dir(dir).with_context(|| format!("ler {}", dir.display()))?;
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().to_string();
        let rel = format!("{prefix}{name}");
        if entry.path().is_dir() {
            collect_files(&entry.path(), &format!("{rel}/"), out)?;
        } else {
            out.push(rel);
        }
    }
    Ok(())
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_digest() {
        assert_eq!(
            hash_bytes(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn dir_hash_tracks_names_and_content() {
        let dir = crate::tmp::TempDir::new("agsync-test").unwrap();
        let d = dir.path();
        std::fs::create_dir_all(d.join("sub")).unwrap();
        std::fs::write(d.join("SKILL.md"), "a").unwrap();
        std::fs::write(d.join("sub/x.txt"), "b").unwrap();
        let first = hash_dir(d).unwrap();
        assert_eq!(hash_dir(d).unwrap(), first);
        std::fs::write(d.join("sub/x.txt"), "c").unwrap();
        let changed = hash_dir(d).unwrap();
        assert_ne!(changed, first);
        std::fs::rename(d.join("sub/x.txt"), d.join("sub/y.txt")).unwrap();
        assert_ne!(hash_dir(d).unwrap(), changed);
    }
}
