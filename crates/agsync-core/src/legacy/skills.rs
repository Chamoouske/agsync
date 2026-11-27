//! Skills that APM or `npx skills` installed: copies in the agents' skills dirs, the
//! canonical copy in `~/.agents/skills` and links to it. Never removed: each one is
//! listed with what `agsync apply --adopt` would do with it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use crate::git::{self, Snapshot};
use crate::machine::Machine;
use crate::manifest::selected;
use crate::paths::Dirs;
use crate::report::read_manifest;

/// The folders the old setup wrote skills to.
fn skill_dirs(home: &Path) -> [PathBuf; 4] {
    [
        home.join(".claude").join("skills"),
        home.join(".config").join("opencode").join("skills"),
        home.join(".hermes").join("skills"),
        home.join(".agents").join("skills"),
    ]
}

/// Prints one line per skill not placed by agsync.
pub fn print(home: &Path, dirs: &Dirs) {
    let found: Vec<(PathBuf, String)> = skill_dirs(home)
        .iter()
        .flat_map(|dir| entries(dir, &dirs.skill_cache()))
        .collect();
    if found.is_empty() {
        return;
    }
    println!("skills de APM ou `npx skills` (nao sao removidas):");
    let wanted = active_names(dirs);
    if let Err(reason) = &wanted {
        println!(
            "  agsync.yaml ativo indisponivel ({reason}); rode `agsync init` para saber quais o agsync assume"
        );
    }
    for (path, kind) in found {
        let name = path.file_name().unwrap_or_default().to_string_lossy();
        let verdict = match &wanted {
            Ok(names) if names.contains(name.as_ref()) => {
                "o agsync.yaml tem esse nome: `agsync apply --adopt` vai assumi-la"
            }
            Ok(_) => "fora do agsync.yaml: fica como entrada manual",
            Err(_) => "sem veredito",
        };
        println!("  {} ({kind}): {verdict}", path.display());
    }
}

/// Skill folders and links in `dir`, with "copia" or "link para <target>", leaving out
/// links into agsync's own cache.
fn entries(dir: &Path, cache: &Path) -> Vec<(PathBuf, String)> {
    let Ok(read) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let cache = std::fs::canonicalize(cache).unwrap_or_else(|_| cache.to_path_buf());
    let mut found = Vec::new();
    for entry in read.flatten() {
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        let target = std::fs::read_link(&path).ok();
        let resolved = std::fs::canonicalize(&path).ok();
        let ours = resolved.is_some_and(|real| real.starts_with(&cache));
        let kind = match target {
            Some(_) if ours => continue,
            Some(target) => format!("link para {}", target.display()),
            None if meta.is_dir() => "copia".to_string(),
            None => continue,
        };
        found.push((path, kind));
    }
    found.sort();
    found
}

/// Names of the skills the active `agsync.yaml` selects for this machine's profiles.
fn active_names(dirs: &Dirs) -> Result<BTreeSet<String>, String> {
    let machine = Machine::load(&dirs.machine_file()).map_err(|e| format!("{e:#}"))?;
    let read = || -> anyhow::Result<BTreeSet<String>> {
        let sha = git::resolve(&machine.repo, &machine.git_ref)?;
        let snapshot = Snapshot::fetch(&machine.repo, &machine.git_ref, &sha)?;
        let manifest = read_manifest(&snapshot)?;
        snapshot.close()?;
        let names = manifest
            .skills
            .iter()
            .filter(|skill| selected(&skill.value.profiles, &machine.profiles))
            .map(|skill| skill.value.name())
            .collect();
        Ok(names)
    };
    read().map_err(|e| format!("{e:#}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_copies_and_foreign_links_but_not_agsync_links() {
        let tmp = crate::tmp::TempDir::new("agsync-test").unwrap();
        let skills = tmp.path().join("skills");
        let cache = tmp.path().join("cache");
        let canonical = tmp.path().join("canonical").join("tdd");
        for dir in [skills.join("copied"), cache.join("ours"), canonical.clone()] {
            std::fs::create_dir_all(&dir).unwrap();
        }
        let backups = tmp.path().join("backups");
        crate::agent::link::install(&canonical, &skills.join("tdd"), &backups).unwrap();
        crate::agent::link::install(&cache.join("ours"), &skills.join("ours"), &backups).unwrap();
        let found = entries(&skills, &cache);
        let names: Vec<String> = found
            .iter()
            .map(|(path, _)| path.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, ["copied", "tdd"]);
        assert_eq!(found[0].1, "copia");
        assert!(found[1].1.starts_with("link para "), "{:?}", found[1]);
    }
}
