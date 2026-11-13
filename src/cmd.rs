//! `init` and `secret` commands.

use std::io::{BufRead, IsTerminal};

use crate::git;
use crate::machine::Machine;
use crate::paths::Dirs;
use crate::secret;

/// Writes `machine.toml` after checking the repo answers `git ls-remote`.
pub fn init(url: String, git_ref: Option<String>, profiles: Vec<String>) -> anyhow::Result<()> {
    let dirs = Dirs::from_env()?;
    let machine = Machine::new(url, git_ref, profiles);
    let commit = git::resolve(&machine.repo, &machine.git_ref)?;
    let file = dirs.machine_file();
    machine.save(&file)?;
    println!(
        "repo ok: {} ({} em {})",
        machine.repo, machine.git_ref, commit
    );
    println!("perfis ativos: {}", machine.profiles.join(", "));
    println!("gravado {}", file.display());
    Ok(())
}

/// Reads a value (no echo on a terminal, else one stdin line) and stores it in `secrets.env`.
pub fn secret_set(name: &str) -> anyhow::Result<()> {
    if !secret::valid_name(name) {
        anyhow::bail!("nome '{name}' invalido (use letras, numeros e _)");
    }
    let dirs = Dirs::from_env()?;
    let value = if std::io::stdin().is_terminal() {
        rpassword::prompt_password(format!("valor de {name}: "))?
    } else {
        let mut line = String::new();
        std::io::stdin().lock().read_line(&mut line)?;
        line.trim_end_matches(['\r', '\n']).to_string()
    };
    if value.is_empty() {
        anyhow::bail!("valor vazio; nada gravado");
    }
    let file = secret::file_path(&dirs.config);
    secret::file::set(&file, name, &value)?;
    println!("{name} gravado em {}", file.display());
    Ok(())
}

/// Prints the names in `secrets.env`, one per line.
pub fn secret_list() -> anyhow::Result<()> {
    let dirs = Dirs::from_env()?;
    for name in secret::file::names(&secret::file_path(&dirs.config))? {
        println!("{name}");
    }
    Ok(())
}
