//! Yes/no questions: asked on the terminal, or answered by `--yes`.

use std::io::{BufRead, IsTerminal, Write};

/// Who answers a yes/no question.
pub trait Confirm {
    /// False when nobody can answer, as on a terminal without a TTY.
    fn can_answer(&self) -> bool;
    /// The answer to `question`.
    fn ask(&self, question: &str) -> anyhow::Result<bool>;
}

/// Asks on the terminal.
pub struct Terminal;

impl Confirm for Terminal {
    fn can_answer(&self) -> bool {
        std::io::stdin().is_terminal()
    }

    fn ask(&self, question: &str) -> anyhow::Result<bool> {
        yes_no(
            question,
            &mut std::io::stdin().lock(),
            &mut std::io::stdout(),
        )
    }
}

/// Says yes without asking: `--yes`, `--silent-update` and `silentUpdate`.
pub struct Yes;

impl Confirm for Yes {
    fn can_answer(&self) -> bool {
        true
    }

    fn ask(&self, _question: &str) -> anyhow::Result<bool> {
        Ok(true)
    }
}

/// [`Yes`] with `yes`, else [`Terminal`].
pub fn from_flag(yes: bool) -> &'static dyn Confirm {
    if yes { &Yes } else { &Terminal }
}

/// Writes `question [s/N] ` and reads one line; only s/sim/y/yes count as yes.
fn yes_no(question: &str, input: &mut impl BufRead, out: &mut impl Write) -> anyhow::Result<bool> {
    write!(out, "{question} [s/N] ")?;
    out.flush()?;
    let mut answer = String::new();
    input.read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_lowercase().as_str(),
        "s" | "sim" | "y" | "yes"
    ))
}

/// Asks `confirm`; when it cannot answer, an error naming `command --yes`; prints
/// "cancelado" on no.
pub fn confirm(confirm: &dyn Confirm, question: &str, command: &str) -> anyhow::Result<bool> {
    if !confirm.can_answer() {
        anyhow::bail!("sem TTY: nada feito; rode `{command} --yes`");
    }
    let ok = confirm.ask(question)?;
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

#[cfg(test)]
mod tests;
