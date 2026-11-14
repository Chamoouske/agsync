//! Linux fallback without a systemd user manager: one line in the user's crontab,
//! found again by the trailing marker.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::Context;

use super::{Scheduler, Status};

/// The crontab line that ends in `# <marker>`.
pub struct Cron {
    pub marker: String,
}

impl Cron {
    pub fn new(marker: &str) -> Cron {
        Cron {
            marker: marker.to_string(),
        }
    }

    /// True when `crontab` is in `PATH`.
    pub fn available() -> bool {
        crate::which::find("crontab").is_some()
    }
}

impl Scheduler for Cron {
    fn install(&self, exe: &Path, minutes: u32) -> anyhow::Result<Vec<String>> {
        let path = super::current_frozen_path();
        let line = cron_line(exe, minutes, &path, &self.marker);
        write_table(&with_line(&read_table(), &line, &self.marker))?;
        let note = super::systemd::frozen_path_note("na linha do crontab", Path::new("crontab -l"));
        Ok(vec![note])
    }

    fn remove(&self) -> anyhow::Result<bool> {
        let table = read_table();
        if !table.lines().any(|l| marked(l, &self.marker)) {
            return Ok(false);
        }
        write_table(&without_line(&table, &self.marker))?;
        Ok(true)
    }

    fn status(&self, exe: &Path) -> anyhow::Result<Status> {
        let table = read_table();
        let Some(line) = table.lines().find(|l| marked(l, &self.marker)) else {
            return Ok(Status::Missing);
        };
        if line.contains(&run_part(exe)) {
            return Ok(Status::Current);
        }
        Ok(Status::Other(line.to_string()))
    }
}

/// `*/N * * * * env PATH='…' '<exe>' check >/dev/null 2>&1 # <marker>`.
pub fn cron_line(exe: &Path, minutes: u32, path: &str, marker: &str) -> String {
    let path = sh_quote(&format!("PATH={path}"));
    format!(
        "{} env {path} {} >/dev/null 2>&1 {}",
        every(minutes),
        run_part(exe),
        comment(marker)
    )
    .replace('%', "\\%")
}

/// Minute field for `minutes` (whole hours past 59 minutes).
pub fn every(minutes: u32) -> String {
    if minutes < 60 {
        return format!("*/{minutes} * * * *");
    }
    format!("0 */{} * * *", (minutes / 60).min(23))
}

/// `table` with any old line of `marker` dropped and `line` appended.
pub fn with_line(table: &str, line: &str, marker: &str) -> String {
    let mut out = without_line(table, marker);
    out.push_str(line);
    out.push('\n');
    out
}

/// `table` without the line of `marker`; every other line kept as is.
pub fn without_line(table: &str, marker: &str) -> String {
    table
        .lines()
        .filter(|l| !marked(l, marker))
        .map(|l| format!("{l}\n"))
        .collect()
}

fn run_part(exe: &Path) -> String {
    format!("{} check", sh_quote(&exe.to_string_lossy()))
}

fn comment(marker: &str) -> String {
    format!("# {marker}")
}

fn marked(line: &str, marker: &str) -> bool {
    line.trim_end().ends_with(&comment(marker))
}

fn sh_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "'\\''"))
}

/// The current crontab; none yet reads as empty.
fn read_table() -> String {
    let out = Command::new("crontab")
        .arg("-l")
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output();
    match out {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).to_string(),
        _ => String::new(),
    }
}

fn write_table(table: &str) -> anyhow::Result<()> {
    let mut child = Command::new("crontab")
        .arg("-")
        .stdin(Stdio::piped())
        .spawn()
        .context("executar crontab")?;
    child
        .stdin
        .take()
        .context("stdin do crontab")?
        .write_all(table.as_bytes())?;
    let status = child.wait()?;
    if !status.success() {
        anyhow::bail!("crontab - saiu com {status}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_runs_check_with_path_and_marker() {
        let exe = Path::new("/home/ana/it's/agsync");
        let line = cron_line(exe, 15, "/usr/bin:/x%y", "agsync-check");
        assert_eq!(
            line,
            "*/15 * * * * env 'PATH=/usr/bin:/x\\%y' '/home/ana/it'\\''s/agsync' check >/dev/null 2>&1 # agsync-check"
        );
        assert_eq!(every(90), "0 */1 * * *");
        assert_eq!(every(120), "0 */2 * * *");
    }

    #[test]
    fn table_keeps_other_lines_and_one_line_per_marker() {
        let table = "MAILTO=x\n0 1 * * * backup\n";
        let one = with_line(table, "*/15 * * * * a # agsync-check", "agsync-check");
        let two = with_line(&one, "*/5 * * * * b # agsync-check", "agsync-check");
        assert_eq!(
            two,
            "MAILTO=x\n0 1 * * * backup\n*/5 * * * * b # agsync-check\n"
        );
        assert_eq!(without_line(&two, "agsync-check"), table);
        assert_eq!(without_line(table, "agsync-check"), table);
        let test = with_line(&two, "* * * * * t # agsync-test-1", "agsync-test-1");
        assert_eq!(without_line(&test, "agsync-test-1"), two);
        assert_eq!(without_line(&test, "agsync-check").lines().count(), 3);
    }
}
