//! `<data>/logs/check.log`: one line per `agsync check` event, renamed to `.1` past 1 MB.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::paths::Dirs;

const MAX_BYTES: u64 = 1024 * 1024;

/// Where the log lives.
pub fn path(dirs: &Dirs) -> PathBuf {
    dirs.logs().join("check.log")
}

/// Appends `<UTC time> <text>`. A log that cannot be written never fails the check.
pub fn write(dirs: &Dirs, text: &str) {
    let line = format!("{} {text}\n", utc_now());
    if let Err(e) = append(&path(dirs), &line, MAX_BYTES) {
        eprintln!("aviso: gravar check.log falhou: {e}");
    }
}

fn append(path: &Path, line: &str, max: u64) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    if size + line.len() as u64 > max && size > 0 {
        std::fs::rename(path, path.with_extension("log.1"))?;
    }
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    file.write_all(line.as_bytes())
}

/// Current time as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn utc_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    utc(secs)
}

fn utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rest = secs % 86_400;
    let (y, m, d) = civil(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// Days since 1970-01-01 to (year, month, day), after Howard Hinnant's `civil_from_days`.
fn civil(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_utc_dates() {
        assert_eq!(utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(utc(1_794_571_199), "2026-11-13T11:59:59Z");
    }

    #[test]
    fn rotates_to_dot_one_past_the_limit() {
        let tmp = crate::tmp::TempDir::new("agsync-test").unwrap();
        let log = tmp.path().join("logs").join("check.log");
        append(&log, "aaaa\n", 12).unwrap();
        append(&log, "bbbb\n", 12).unwrap();
        append(&log, "cccc\n", 12).unwrap();
        let old = tmp.path().join("logs").join("check.log.1");
        assert_eq!(std::fs::read_to_string(&old).unwrap(), "aaaa\nbbbb\n");
        assert_eq!(std::fs::read_to_string(&log).unwrap(), "cccc\n");
        append(&log, "dddd\n", 12).unwrap();
        append(&log, "eeee\n", 12).unwrap();
        assert_eq!(std::fs::read_to_string(&old).unwrap(), "cccc\ndddd\n");
    }
}
