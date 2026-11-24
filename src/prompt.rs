//! Yes/no questions on the terminal.

use std::io::{BufRead, IsTerminal, Write};

/// Prints `question [s/N] ` and reads one line; only s/sim/y/yes count as yes.
pub fn yes_no(question: &str) -> anyhow::Result<bool> {
    print!("{question} [s/N] ");
    std::io::stdout().flush()?;
    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_lowercase().as_str(),
        "s" | "sim" | "y" | "yes"
    ))
}

/// True with `yes`; without a terminal, an error naming `command --yes`; else asks
/// `question` and prints "cancelado" on no.
pub fn confirm(yes: bool, question: &str, command: &str) -> anyhow::Result<bool> {
    if yes {
        return Ok(true);
    }
    if !std::io::stdin().is_terminal() {
        anyhow::bail!("sem TTY: nada feito; rode `{command} --yes`");
    }
    let ok = yes_no(question)?;
    if !ok {
        println!("cancelado");
    }
    Ok(ok)
}

/// Prints `question: ` and reads lines until one is not blank; errors when stdin ends.
pub fn ask(question: &str) -> anyhow::Result<String> {
    loop {
        print!("{question}: ");
        std::io::stdout().flush()?;
        let mut answer = String::new();
        let read = std::io::stdin().lock().read_line(&mut answer)?;
        if read == 0 {
            println!();
            anyhow::bail!("entrada encerrada sem resposta para \"{question}\"");
        }
        let answer = answer.trim();
        if !answer.is_empty() {
            return Ok(answer.to_string());
        }
    }
}
