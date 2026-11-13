//! Running an agent CLI: one blocking call at a time, stdin closed.

use std::process::{Command, Stdio};

use anyhow::Context;

/// Runs `cmd` and waits for it. On failure the error names `what` and the exit code;
/// the CLI output is only included when `show_output` is set, because arguments and
/// echoes may hold secret values.
pub fn run(mut cmd: Command, what: &str, show_output: bool) -> anyhow::Result<()> {
    let out = cmd
        .stdin(Stdio::null())
        .output()
        .with_context(|| format!("executar {what}"))?;
    if out.status.success() {
        return Ok(());
    }
    let code = out.status.code().unwrap_or(-1);
    if !show_output {
        anyhow::bail!("{what} falhou (exit {code})");
    }
    let text = format!(
        "{} {}",
        String::from_utf8_lossy(&out.stdout).trim(),
        String::from_utf8_lossy(&out.stderr).trim()
    );
    anyhow::bail!("{what} falhou (exit {code}): {}", text.trim());
}
