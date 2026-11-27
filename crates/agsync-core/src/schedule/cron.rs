//! Linux fallback without a systemd user manager: one line in the user's crontab,
//! found again by the trailing marker.
//!
//! The line only runs `<data>/<marker>.sh`, a script agsync writes with the frozen
//! `PATH` and the binary. cron refuses long lines, and a `PATH` of any size or content
//! fits in the script.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::Context;

use super::{Scheduler, Status};

/// The crontab line that ends in `# <marker>` and the script it runs.
pub struct Cron {
    pub marker: String,
    pub script: PathBuf,
}

impl Cron {
    /// The script is `<dir>/<marker>.sh`.
    pub fn new(marker: &str, dir: &Path) -> Cron {
        Cron {
            marker: marker.to_string(),
            script: dir.join(format!("{marker}.sh")),
        }
    }

    /// True when `crontab` is in `PATH`.
    pub fn available() -> bool {
        crate::which::find("crontab").is_some()
    }
}

impl Scheduler for Cron {
    fn install(&self, exe: &Path, minutes: u32) -> anyhow::Result<Vec<String>> {
        let text = script_text(exe, &super::current_frozen_path());
        write_script(&self.script, &text)?;
        let line = cron_line(&self.script, minutes, &self.marker);
        write_table(&with_line(&read_table(), &line, &self.marker))?;
        let note = super::systemd::frozen_path_note("no script do cron", &self.script);
        Ok(vec![note])
    }

    fn remove(&self) -> anyhow::Result<bool> {
        let table = read_table();
        let had_line = table.lines().any(|l| marked(l, &self.marker));
        if had_line {
            write_table(&without_line(&table, &self.marker))?;
        }
        let had_script = self.script.exists();
        if had_script {
            std::fs::remove_file(&self.script)
                .with_context(|| format!("apagar {}", self.script.display()))?;
        }
        Ok(had_line || had_script)
    }

    fn status(&self, exe: &Path) -> anyhow::Result<Status> {
        let table = read_table();
        let Some(line) = table.lines().find(|l| marked(l, &self.marker)) else {
            return Ok(Status::Missing);
        };
        if !line.contains(&sh_quote(&self.script.to_string_lossy())) {
            return Ok(Status::Other(line.to_string()));
        }
        let script = std::fs::read_to_string(&self.script).unwrap_or_default();
        if script.lines().any(|l| l == exec_line(exe)) {
            return Ok(Status::Current);
        }
        let found = script.lines().find(|l| l.starts_with("exec "));
        let found = found.unwrap_or("script ausente");
        Ok(Status::Other(format!("{}: {found}", self.script.display())))
    }
}

/// `*/N * * * * '<script>' >/dev/null 2>&1 # <marker>`.
pub fn cron_line(script: &Path, minutes: u32, marker: &str) -> String {
    format!(
        "{} {} >/dev/null 2>&1 {}",
        every(minutes),
        sh_quote(&script.to_string_lossy()),
        comment(marker)
    )
    .replace('%', "\\%")
}

/// The script the line runs: the frozen `PATH`, then `exec '<exe>' check`.
pub fn script_text(exe: &Path, path: &str) -> String {
    format!(
        "#!/bin/sh\nPATH={}\nexport PATH\n{}\n",
        sh_quote(path),
        exec_line(exe)
    )
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

fn exec_line(exe: &Path) -> String {
    format!("exec {} check", sh_quote(&exe.to_string_lossy()))
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

/// Writes the script with mode 0700: only the user reads and runs it.
fn write_script(script: &Path, text: &str) -> anyhow::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    crate::atomic::write_atomic(script, text.as_bytes())?;
    std::fs::set_permissions(script, std::fs::Permissions::from_mode(0o700))
        .with_context(|| format!("chmod 700 {}", script.display()))
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

    /// A `PATH` of more than 2000 characters, with spaces, parentheses and quotes.
    fn long_path() -> String {
        let dirs: Vec<String> = (0..60)
            .map(|i| format!("/opt/Program Files (x86)/it's tool {i}/bin"))
            .collect();
        let path = dirs.join(":");
        assert!(path.len() > 2000, "{}", path.len());
        path
    }

    #[test]
    fn line_only_runs_the_script() {
        let line = cron_line(
            Path::new("/home/ana/it's/agsync-check.sh"),
            15,
            "agsync-check",
        );
        assert_eq!(
            line,
            "*/15 * * * * '/home/ana/it'\\''s/agsync-check.sh' >/dev/null 2>&1 # agsync-check"
        );
        assert_eq!(every(90), "0 */1 * * *");
        assert_eq!(every(120), "0 */2 * * *");
        let odd = cron_line(Path::new("/data/50%/a.sh"), 5, "m");
        assert!(odd.contains("'/data/50\\%/a.sh'"), "{odd}");
    }

    #[test]
    fn script_keeps_a_long_path_exactly() {
        let tmp = crate::tmp::TempDir::new("agsync-test").unwrap();
        let echo = tmp.path().join("echo path");
        std::fs::write(&echo, "#!/bin/sh\nprintf '%s|%s' \"$1\" \"$PATH\"\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&echo, std::fs::Permissions::from_mode(0o755)).unwrap();
        let path = long_path();
        let script = tmp.path().join("data dir").join("agsync-test-x.sh");
        write_script(&script, &script_text(&echo, &path)).unwrap();
        let mode = std::fs::metadata(&script).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700);
        let line = cron_line(&script, 15, "agsync-test-x");
        assert!(line.len() < 200, "{line}");
        let out = Command::new(&script)
            .env("PATH", "/usr/bin:/bin")
            .output()
            .unwrap();
        assert!(out.status.success());
        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            format!("check|{path}")
        );
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
