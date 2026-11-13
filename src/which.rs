//! Executable lookup in `PATH`, honoring `PATHEXT` on Windows.

use std::path::{Path, PathBuf};

/// First executable named `name` in `PATH`.
pub fn find(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    let exts = extensions(name);
    std::env::split_paths(&path)
        .flat_map(|dir| exts.iter().map(move |ext| dir.join(format!("{name}{ext}"))))
        .find(|candidate| is_executable(candidate))
}

/// Suffixes to try: `PATHEXT` entries on Windows (plus the bare name if it already has
/// an extension); just the bare name elsewhere.
fn extensions(name: &str) -> Vec<String> {
    if !cfg!(windows) {
        return vec![String::new()];
    }
    let pathext = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
    let mut exts: Vec<String> = pathext
        .split(';')
        .filter(|e| !e.is_empty())
        .map(|e| e.to_lowercase())
        .collect();
    if Path::new(name).extension().is_some() {
        exts.insert(0, String::new());
    }
    exts
}

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}

#[cfg(windows)]
fn is_executable(path: &Path) -> bool {
    path.is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_git_and_not_nonsense() {
        assert!(find("git").is_some());
        assert!(find("agsync-no-such-tool-xyz").is_none());
    }

    #[cfg(windows)]
    #[test]
    fn windows_uses_pathext_and_explicit_extension() {
        let exts = extensions("node");
        assert!(exts.contains(&".exe".to_string()), "{exts:?}");
        assert!(!exts.contains(&String::new()));
        assert_eq!(extensions("tool.exe")[0], "");
        assert!(find("git.exe").is_some());
    }
}
