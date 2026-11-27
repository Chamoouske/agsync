//! Questions asked on the terminal: yes/no for [`Confirm`], and free text for `init`.

use std::io::{BufRead, IsTerminal, Write};

use agsync_core::prompt::{Confirm, yes_or};

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

/// [`agsync_core::prompt::Yes`] with `yes`, else [`Terminal`].
pub fn from_flag(yes: bool) -> &'static dyn Confirm {
    yes_or(yes, &Terminal)
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
mod tests {
    use super::*;

    #[test]
    fn yes_flag_answers_without_a_terminal() {
        assert!(from_flag(true).can_answer());
    }

    #[test]
    fn terminal_question_reads_one_line() {
        for (line, expected) in [
            ("s\n", true),
            ("SIM\n", true),
            ("y\n", true),
            ("yes\n", true),
            ("n\n", false),
            ("\n", false),
            ("", false),
        ] {
            let mut out = Vec::new();
            let answer = yes_no("Aplicar estas mudancas?", &mut line.as_bytes(), &mut out).unwrap();
            assert_eq!(answer, expected, "{line:?}");
            assert_eq!(
                String::from_utf8(out).unwrap(),
                "Aplicar estas mudancas? [s/N] "
            );
        }
    }
}
