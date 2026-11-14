//! Executable lookup in `PATH`, honoring `PATHEXT` on Windows. Executables of another
//! OS reachable from here (Windows programs seen from WSL) are skipped.

use std::path::{Path, PathBuf};

/// First executable named `name` in `PATH` that belongs to this OS.
pub fn find(name: &str) -> Option<PathBuf> {
    let host = crate::host::current();
    candidates(&extensions(name), name)
        .into_iter()
        .find(|exe| !host.foreign(exe))
}

/// First executable named `name` (or `name.exe`) that [`find`] skipped as foreign.
pub fn foreign(name: &str) -> Option<PathBuf> {
    let host = crate::host::current();
    let mut found = candidates(&extensions(name), name);
    found.extend(candidates(&[String::new()], &format!("{name}.exe")));
    found.into_iter().find(|exe| host.foreign(exe))
}

/// [`find`], or why `name` is not used.
pub fn require(name: &str) -> Result<PathBuf, String> {
    if let Some(exe) = find(name) {
        return Ok(exe);
    }
    match foreign(name) {
        Some(exe) => Err(format!(
            "{name} achado so no Windows (interop), ignorado: {}",
            exe.display()
        )),
        None => Err(format!("{name} nao encontrado no PATH")),
    }
}

/// Executables `<dir>/<name><ext>` for every `PATH` dir and ext, in lookup order.
fn candidates(exts: &[String], name: &str) -> Vec<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    let mut found = Vec::new();
    for dir in std::env::split_paths(&path) {
        for ext in exts {
            let candidate = dir.join(format!("{name}{ext}"));
            if is_executable(&candidate) {
                found.push(candidate);
            }
        }
    }
    found
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
