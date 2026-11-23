//! `agsync init`: machine.toml, repo access, schedule, shell hook, first apply.
//!
//! Each step reports its own failure; later steps still run when they make sense.

use crate::git;
use crate::machine::Machine;
use crate::paths::Dirs;

/// Runs every step. Returns false when a step failed. A missing `url` or empty
/// `profiles` is asked on the terminal; given values are kept as they are.
pub fn init(
    url: Option<String>,
    git_ref: Option<String>,
    profiles: Vec<String>,
    yes: bool,
    silent: bool,
) -> anyhow::Result<bool> {
    let dirs = Dirs::from_env()?;
    let url = match url {
        Some(url) => url,
        None => crate::prompt::ask("URL do repo de config")?,
    };
    let profiles = if profiles.is_empty() {
        split_profiles(&crate::prompt::ask(
            "Perfis desta maquina (separados por virgula)",
        )?)
    } else {
        profiles
    };
    let machine = Machine::new(url, git_ref, profiles);
    let file = dirs.machine_file();
    println!("[1/5] machine.toml");
    machine.save(&file)?;
    println!("gravado {}", file.display());
    println!("perfis ativos: {}", machine.profiles.join(", "));

    println!("[2/5] acesso ao repo");
    let reachable = match git::resolve(&machine.repo, &machine.git_ref) {
        Ok(commit) => {
            println!(
                "repo ok: {} ({} em {commit})",
                machine.repo, machine.git_ref
            );
            true
        }
        Err(e) => {
            let text = format!("{e:#}");
            let hint = crate::doctor::hint(&text)
                .map(|h| format!("; {h}"))
                .unwrap_or_default();
            eprintln!("repo inacessivel: {text}{hint}");
            false
        }
    };

    println!("[3/5] agendamento do check");
    if let Err(e) = crate::schedule::install() {
        eprintln!("aviso: agendamento nao registrado: {e:#}");
        eprintln!("o `agsync hook` dispara o check quando o shell abre");
    }

    println!("[4/5] hook de shell");
    let hooked = crate::shell::install(yes).unwrap_or_else(|e| {
        eprintln!("hook de shell falhou: {e:#}");
        false
    });

    println!("[5/5] primeiro apply");
    let applied = first_apply(reachable, yes, silent);
    Ok(reachable && hooked && applied)
}

fn split_profiles(answer: &str) -> Vec<String> {
    answer
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

fn first_apply(reachable: bool, yes: bool, silent: bool) -> bool {
    if !reachable {
        println!("pulado: repo inacessivel; corrija e rode `agsync apply`");
        return true;
    }
    match crate::apply::init_apply(yes, silent) {
        Ok(true) => true,
        Ok(false) => {
            println!("pulado (sem TTY): rode `agsync apply --yes`");
            true
        }
        Err(e) => {
            eprintln!("apply falhou: {e:#}");
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_profiles_accepts_commas_and_spaces() {
        assert_eq!(
            split_profiles(" work, personal  ci,,"),
            ["work", "personal", "ci"]
        );
    }
}
