//! Shell profiles that run `agsync hook` at startup: one small interface, one module
//! per shell. `install` and `remove` are reusable by `init` and `uninstall`.

use std::path::{Path, PathBuf};

use anyhow::Context;
use serde::Serialize;

use crate::paths::Dirs;

mod bash;
pub mod block;
mod fish;
mod powershell;
mod zsh;

/// One shell agsync can hook into.
pub trait Shell {
    /// Name shown to the user.
    fn name(&self) -> &'static str;
    /// True when the shell is installed on this machine.
    fn present(&self) -> bool;
    /// File that gets the block, or `None` when it cannot be found.
    fn profile(&self) -> Option<PathBuf>;
    /// Lines that run `exe hook`, in this shell's syntax.
    fn body(&self, exe: &Path) -> String;
}

/// Every shell agsync knows.
pub fn all() -> Vec<Box<dyn Shell>> {
    vec![
        Box::new(powershell::PowerShell::windows()),
        Box::new(powershell::PowerShell::core()),
        Box::new(bash::Bash),
        Box::new(zsh::Zsh),
        Box::new(fish::Fish),
    ]
}

/// Offers the hook to each shell on this machine, one y/n question per shell. Without a
/// terminal nothing is installed unless `yes`. Returns false when a profile failed.
pub fn install(confirm: &dyn crate::prompt::Confirm) -> anyhow::Result<bool> {
    if !confirm.can_answer() {
        println!("sem TTY: hook de shell nao instalado; rode `agsync hook install --yes`");
        return Ok(true);
    }
    let mut ok = true;
    offer(confirm, &mut |hooked| {
        hooked.print();
        ok &= !matches!(hooked, Hooked::Failed { .. });
    })?;
    Ok(ok)
}

/// What happened to the hook of one shell.
#[derive(Serialize, Debug, Clone, PartialEq, Eq)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Hooked {
    NoProfile { shell: String },
    Declined { shell: String, profile: PathBuf },
    Installed { shell: String, profile: PathBuf },
    AlreadyThere { shell: String, profile: PathBuf },
    Failed { shell: String, reason: String },
}

impl Hooked {
    /// The line `agsync hook install` prints; none for a declined shell.
    pub fn print(&self) {
        match self {
            Hooked::NoProfile { shell } => {
                eprintln!("{shell}: perfil nao encontrado; hook nao instalado")
            }
            Hooked::Declined { .. } => {}
            Hooked::Installed { shell, profile } => {
                println!("{shell}: hook instalado em {}", profile.display())
            }
            Hooked::AlreadyThere { shell, profile } => {
                println!("{shell}: hook ja estava em {}", profile.display())
            }
            Hooked::Failed { shell, reason } => eprintln!("{shell}: hook nao instalado: {reason}"),
        }
    }
}

/// Asks `confirm` once per shell on this machine and installs the hook where it says
/// yes, without printing. `each` gets every shell's outcome right after it happens.
pub fn offer(
    confirm: &dyn crate::prompt::Confirm,
    each: &mut dyn FnMut(Hooked),
) -> anyhow::Result<()> {
    let exe = crate::paths::current_exe()?;
    let backups = Dirs::from_env()?.backups();
    for shell in all().into_iter().filter(|s| s.present()) {
        let name = shell.name().to_string();
        let Some(profile) = shell.profile() else {
            each(Hooked::NoProfile { shell: name });
            continue;
        };
        let question = format!(
            "Instalar o hook do agsync no {name} ({})?",
            profile.display()
        );
        if !confirm.ask(&question)? {
            each(Hooked::Declined {
                shell: name,
                profile,
            });
            continue;
        }
        let hooked = match install_into(&profile, &shell.body(&exe), &backups) {
            Ok(true) => Hooked::Installed {
                shell: name,
                profile,
            },
            Ok(false) => Hooked::AlreadyThere {
                shell: name,
                profile,
            },
            Err(e) => Hooked::Failed {
                shell: name,
                reason: format!("{e:#}"),
            },
        };
        each(hooked);
    }
    Ok(())
}

/// Removes the block from every known profile that has it.
pub fn remove() -> anyhow::Result<()> {
    let backups = Dirs::from_env()?.backups();
    let mut removed = 0;
    for shell in all() {
        let Some(profile) = shell.profile() else {
            continue;
        };
        if remove_from(&profile, &backups)? {
            removed += 1;
            println!("{}: hook removido de {}", shell.name(), profile.display());
        }
    }
    if removed == 0 {
        println!("nenhum hook de shell encontrado");
    }
    Ok(())
}

/// Writes the block into `profile` (backup first, atomic write). False if already there.
pub fn install_into(profile: &Path, body: &str, backups: &Path) -> anyhow::Result<bool> {
    let old = read(profile)?;
    let new = block::with_block(&old, body);
    if new == old {
        return Ok(false);
    }
    backup(profile, backups)?;
    crate::atomic::write_atomic(profile, new.as_bytes())?;
    Ok(true)
}

/// Takes the block out of `profile`; a file left empty is deleted. False if absent.
pub fn remove_from(profile: &Path, backups: &Path) -> anyhow::Result<bool> {
    let old = read(profile)?;
    if !block::has_block(&old) {
        return Ok(false);
    }
    backup(profile, backups)?;
    let new = block::without_block(&old);
    if new.trim().is_empty() {
        std::fs::remove_file(profile).with_context(|| format!("apagar {}", profile.display()))?;
    } else {
        crate::atomic::write_atomic(profile, new.as_bytes())?;
    }
    Ok(true)
}

fn read(profile: &Path) -> anyhow::Result<String> {
    match std::fs::read_to_string(profile) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e).with_context(|| format!("ler {}", profile.display())),
    }
}

fn backup(profile: &Path, backups: &Path) -> anyhow::Result<()> {
    let name = profile.file_name().unwrap_or_default().to_string_lossy();
    crate::agent::backup::copy(profile, backups, &format!("shell-{name}"))
}

/// `'text'` for POSIX shells.
fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_and_remove_keep_the_user_lines() {
        let tmp = crate::tmp::TempDir::new("agsync-test").unwrap();
        let profile = tmp.path().join("home").join(".bashrc");
        let backups = tmp.path().join("backups");
        std::fs::create_dir_all(profile.parent().unwrap()).unwrap();
        std::fs::write(&profile, "export A=1\n").unwrap();
        assert!(install_into(&profile, "run", &backups).unwrap());
        assert!(!install_into(&profile, "run", &backups).unwrap());
        assert_eq!(std::fs::read_dir(&backups).unwrap().count(), 1);
        assert!(remove_from(&profile, &backups).unwrap());
        assert!(!remove_from(&profile, &backups).unwrap());
        assert_eq!(std::fs::read_to_string(&profile).unwrap(), "export A=1\n");

        let fresh = tmp.path().join("conf.d").join("agsync.fish");
        assert!(install_into(&fresh, "run", &backups).unwrap());
        assert!(remove_from(&fresh, &backups).unwrap());
        assert!(!fresh.exists());
    }
}
