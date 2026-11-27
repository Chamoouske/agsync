//! `agsync uninstall`: takes every entry of `state.json` out of the agents, removes the
//! schedule and the shell hook, and deletes the state. `--purge` also deletes the
//! config and data dirs. The binary itself stays.

use std::path::Path;

use anyhow::Context;

use crate::agent::{self, Adapter};
use crate::lock::Lock;
use crate::paths::Dirs;
use crate::state::{AgentState, State};

/// Runs the uninstall after one confirmation. False when some step failed.
pub fn uninstall(confirm: &dyn crate::prompt::Confirm, purge: bool) -> anyhow::Result<bool> {
    let dirs = Dirs::from_env()?;
    let exe = crate::paths::current_exe()?;
    print_plan(&dirs, purge);
    let question = "Desinstalar o agsync desta maquina?";
    if !crate::prompt::confirm(confirm, question, "agsync uninstall")? {
        return Ok(true);
    }
    let lock = Lock::wait(&dirs)?;
    let entries_ok = remove_entries(&dirs)?;
    let schedule_ok = step("agendamento", crate::schedule::remove());
    let hook_ok = step("hook de shell", crate::shell::remove());
    if entries_ok {
        delete_local_state(&dirs)?;
    }
    drop(lock);
    if purge && entries_ok {
        purge_dir(&dirs.config, &exe)?;
        purge_dir(&dirs.data, &exe)?;
        println!(
            "apagados {} e {}",
            dirs.config.display(),
            dirs.data.display()
        );
    }
    if !entries_ok {
        eprintln!(
            "entradas que falharam continuam em {}; rode `agsync uninstall` de novo",
            dirs.state_file().display()
        );
    }
    println!(
        "o binario continua em {}; apague-o a mao e tire a pasta dele do PATH",
        exe.display()
    );
    Ok(entries_ok && schedule_ok && hook_ok)
}

fn print_plan(dirs: &Dirs, purge: bool) {
    println!("agsync uninstall vai:");
    println!("  remover dos agentes cada mcp, skill e plugin do state.json");
    println!("  remover o agendamento do `agsync check` e o hook de todos os shells");
    println!("  apagar state.json e pending.json");
    if purge {
        println!();
        println!(
            "!!! --purge: apaga tambem {} e {}, com secrets.env, cache e TODOS OS BACKUPS !!!",
            dirs.config.display(),
            dirs.data.display()
        );
        println!();
    }
}

fn step(what: &str, result: anyhow::Result<()>) -> bool {
    match result {
        Ok(()) => true,
        Err(e) => {
            eprintln!("{what}: {e:#}");
            false
        }
    }
}

/// Removes each tracked entry through its adapter; keeps in the state only the ones
/// that failed. An agent no longer found loses its entries from the state only.
fn remove_entries(dirs: &Dirs) -> anyhow::Result<bool> {
    let file = dirs.state_file();
    let mut state = State::load(&file)?;
    let mut ok = true;
    for (name, detect) in agent::AGENTS {
        let Some(entries) = state.agents.remove(name) else {
            continue;
        };
        let count = entries.mcps.len() + entries.skills.len() + entries.plugins.len();
        if count == 0 {
            continue;
        }
        let left = match detect() {
            Ok(adapter) => remove_agent(name, adapter.as_ref(), entries, dirs),
            Err(reason) => {
                eprintln!("{name}: {reason}; {count} entrada(s) ficam na config dele");
                AgentState::default()
            }
        };
        if left.mcps.len() + left.skills.len() + left.plugins.len() > 0 {
            ok = false;
            state.agents.insert(name.to_string(), left);
        }
    }
    if !ok {
        state.save(&file)?;
    }
    Ok(ok)
}

/// Entries that could not be removed.
fn remove_agent(name: &str, adapter: &dyn Adapter, entries: AgentState, dirs: &Dirs) -> AgentState {
    let backups = dirs.backups();
    if let Err(e) = adapter.backup(&backups) {
        eprintln!("{name}: backup falhou, agente pulado: {e:#}");
        return entries;
    }
    let mut left = AgentState::default();
    for (mcp, tracked) in entries.mcps {
        match adapter.remove_mcp(&mcp) {
            Ok(()) => println!("{name}: mcp {mcp}: removido"),
            Err(e) => {
                eprintln!("{name}: mcp {mcp} falhou: {e:#}");
                left.mcps.insert(mcp, tracked);
            }
        }
    }
    for (skill, tracked) in entries.skills {
        match adapter.remove_skill(&skill, &backups) {
            Ok(()) => println!("{name}: skill {skill}: removida"),
            Err(e) => {
                eprintln!("{name}: skill {skill} falhou: {e:#}");
                left.skills.insert(skill, tracked);
            }
        }
    }
    for (plugin, tracked) in entries.plugins {
        let Some(id) = tracked.plugin.clone() else {
            continue;
        };
        match adapter.uninstall_plugin(&id) {
            Ok(()) => println!("{name}: plugin {plugin}: removido"),
            Err(e) => {
                eprintln!("{name}: plugin {plugin} falhou: {e:#}");
                left.plugins.insert(plugin, tracked);
            }
        }
    }
    left
}

fn delete_local_state(dirs: &Dirs) -> anyhow::Result<()> {
    for file in [dirs.state_file(), dirs.pending_file(), dirs.watch_file()] {
        match std::fs::remove_file(&file) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                return Err(e).with_context(|| format!("apagar {}", file.display()));
            }
            _ => {}
        }
    }
    println!("apagados state.json e pending.json");
    Ok(())
}

/// Deletes `dir`, except the folder that holds `keep` (the binary may live in the
/// data dir, as `%LOCALAPPDATA%\agsync\bin` does).
fn purge_dir(dir: &Path, keep: &Path) -> anyhow::Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    if !keep.starts_with(dir) {
        return std::fs::remove_dir_all(dir).with_context(|| format!("apagar {}", dir.display()));
    }
    let entries = std::fs::read_dir(dir).with_context(|| format!("ler {}", dir.display()))?;
    for entry in entries {
        let path = entry?.path();
        if keep.starts_with(&path) {
            continue;
        }
        let removed = if path.is_dir() {
            std::fs::remove_dir_all(&path)
        } else {
            std::fs::remove_file(&path)
        };
        removed.with_context(|| format!("apagar {}", path.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn purge_keeps_the_folder_of_the_binary() {
        let tmp = crate::tmp::TempDir::new("agsync-test").unwrap();
        let data = tmp.path().join("agsync");
        let bin = data.join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::create_dir_all(data.join("backups")).unwrap();
        std::fs::write(data.join("state.json"), "{}").unwrap();
        std::fs::write(bin.join("agsync.exe"), "x").unwrap();
        purge_dir(&data, &bin.join("agsync.exe")).unwrap();
        let left: Vec<_> = std::fs::read_dir(&data)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left, ["bin"]);

        let config = tmp.path().join("config");
        std::fs::create_dir_all(&config).unwrap();
        purge_dir(&config, &bin.join("agsync.exe")).unwrap();
        assert!(!config.exists());
    }
}
