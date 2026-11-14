//! Yes/no questions on the terminal.

use std::io::{BufRead, Write};

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
