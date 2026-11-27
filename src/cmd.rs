//! `secret` commands.

use std::io::{BufRead, IsTerminal};

use crate::paths::Dirs;
use crate::secret;

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
    for entry in secret::file::list(&secret::file_path(&dirs.config))? {
        println!("{}", entry.name);
    }
    Ok(())
}
