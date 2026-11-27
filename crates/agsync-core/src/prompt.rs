//! Yes/no questions: answered by whoever implements [`Confirm`], or by `--yes`.

/// Who answers a yes/no question.
pub trait Confirm {
    /// False when nobody can answer, as on a terminal without a TTY.
    fn can_answer(&self) -> bool;
    /// The answer to `question`.
    fn ask(&self, question: &str) -> anyhow::Result<bool>;
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

/// [`Yes`] with `yes`, else `asker`.
pub fn yes_or(yes: bool, asker: &dyn Confirm) -> &dyn Confirm {
    if yes { &Yes } else { asker }
}

/// Asks `confirm`; when it cannot answer, an error naming `command --yes`; prints
/// "cancelado" on no.
pub fn confirm(confirm: &dyn Confirm, question: &str, command: &str) -> anyhow::Result<bool> {
    let ok = answer(confirm, question, command)?;
    if !ok {
        println!("cancelado");
    }
    Ok(ok)
}

/// Asks `confirm`, printing nothing; when it cannot answer, an error naming
/// `command --yes`.
pub fn answer(confirm: &dyn Confirm, question: &str, command: &str) -> anyhow::Result<bool> {
    if !confirm.can_answer() {
        anyhow::bail!("sem TTY: nada feito; rode `{command} --yes`");
    }
    confirm.ask(question)
}

#[cfg(test)]
mod tests;
