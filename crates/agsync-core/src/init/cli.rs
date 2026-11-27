//! `agsync init` on the terminal: asks what is missing, then prints each step as it runs.

use super::{Access, FirstApply, Options, Saved, Scheduling};
use crate::machine::Machine;
use crate::paths::Dirs;
use crate::prompt::Confirm;

/// Runs every step. Returns false when a step failed. A missing `url` or empty
/// `profiles` is asked with `ask`; given values are kept as they are. `asker` answers
/// the shell hook and first apply questions unless `yes` is set.
pub fn init(
    asker: &dyn Confirm,
    ask: fn(&str) -> anyhow::Result<String>,
    url: Option<String>,
    git_ref: Option<String>,
    profiles: Vec<String>,
    yes: bool,
    silent: bool,
) -> anyhow::Result<bool> {
    let dirs = Dirs::from_env()?;
    let url = match url {
        Some(url) => url,
        None => ask("URL do repo de config")?,
    };
    let profiles = if profiles.is_empty() {
        split_profiles(&ask("Perfis desta maquina (separados por virgula)")?)
    } else {
        profiles
    };
    let options = Options {
        url,
        git_ref,
        profiles,
        yes,
        silent,
    };

    println!("[1/5] machine.toml");
    let saved = super::save(&dirs, &options)?;
    print_saved(&saved);

    println!("[2/5] acesso ao repo");
    let access = super::check_access(&saved.machine);
    print_access(&saved.machine, &access);

    println!("[3/5] agendamento do check");
    print_schedule(&super::schedule());

    println!("[4/5] hook de shell");
    let hooked = crate::shell::install(crate::prompt::yes_or(yes, asker)).unwrap_or_else(|e| {
        eprintln!("hook de shell falhou: {e:#}");
        false
    });

    println!("[5/5] primeiro apply");
    let apply = super::first_apply(&access, asker, yes, silent);
    print_apply(&apply);

    let reachable = matches!(access, Access::Reachable { .. });
    Ok(reachable && hooked && apply.ok())
}

fn split_profiles(answer: &str) -> Vec<String> {
    answer
        .split(|c: char| c == ',' || c.is_whitespace())
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect()
}

fn print_saved(saved: &Saved) {
    println!("gravado {}", saved.file.display());
    println!("perfis ativos: {}", saved.machine.profiles.join(", "));
}

fn print_access(machine: &Machine, access: &Access) {
    match access {
        Access::Reachable { commit } => {
            println!(
                "repo ok: {} ({} em {commit})",
                machine.repo, machine.git_ref
            );
        }
        Access::Unreachable { reason, hint } => {
            let hint = hint.map(|h| format!("; {h}")).unwrap_or_default();
            eprintln!("repo inacessivel: {reason}{hint}");
        }
    }
}

fn print_schedule(schedule: &Scheduling) {
    match schedule {
        Scheduling::Scheduled(scheduled) => scheduled.print(),
        Scheduling::Failed { reason } => {
            eprintln!("aviso: agendamento nao registrado: {reason}");
            eprintln!("o `agsync hook` dispara o check quando o shell abre");
        }
    }
}

fn print_apply(apply: &FirstApply) {
    match apply {
        FirstApply::Applied => {}
        FirstApply::Unreachable => {
            println!("pulado: repo inacessivel; corrija e rode `agsync apply`");
        }
        FirstApply::NoTty => println!("pulado (sem TTY): rode `agsync apply --yes`"),
        FirstApply::Failed { reason } => eprintln!("apply falhou: {reason}"),
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
