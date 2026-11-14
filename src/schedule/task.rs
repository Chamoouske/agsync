//! Windows: a Task Scheduler task of the current user, created without admin rights.
//!
//! The action is `conhost.exe --headless "<agsync.exe>" check`. A console program
//! started by Task Scheduler gets a visible console window; under a headless conhost
//! it gets a console without a window, and the `git` it runs shares that console, so
//! nothing flashes on screen every interval.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::Context;

use super::{Scheduler, Status};

pub struct TaskScheduler {
    name: String,
}

impl TaskScheduler {
    pub fn new(name: &str) -> TaskScheduler {
        TaskScheduler {
            name: name.to_string(),
        }
    }

    /// The task as `schtasks /query /xml` prints it, or `None` when it does not exist.
    pub fn query_xml(&self) -> Option<String> {
        let out = schtasks(&["/query", "/tn", &self.name, "/xml", "ONE"]).ok()?;
        Some(out)
    }
}

impl Scheduler for TaskScheduler {
    fn install(&self, exe: &Path, minutes: u32) -> anyhow::Result<Vec<String>> {
        let user = user_id()?;
        let xml = task_xml(&user, exe, minutes, &crate::checklog::utc_now());
        let dir = crate::tmp::TempDir::new("agsync-task")?;
        let file = dir.path().join("task.xml");
        std::fs::write(&file, utf16_with_bom(&xml))
            .with_context(|| format!("gravar {}", file.display()))?;
        let file = file.to_string_lossy().to_string();
        schtasks(&["/create", "/tn", &self.name, "/xml", &file, "/f"])?;
        Ok(Vec::new())
    }

    fn remove(&self) -> anyhow::Result<bool> {
        if self.query_xml().is_none() {
            return Ok(false);
        }
        schtasks(&["/delete", "/tn", &self.name, "/f"])?;
        Ok(true)
    }

    fn status(&self, exe: &Path) -> anyhow::Result<Status> {
        let Some(xml) = self.query_xml() else {
            return Ok(Status::Missing);
        };
        let command = tag(&xml, "Command").unwrap_or_default();
        let arguments = tag(&xml, "Arguments").unwrap_or_default();
        let headless = command.eq_ignore_ascii_case(&conhost().to_string_lossy());
        if headless && arguments == check_args(exe) {
            return Ok(Status::Current);
        }
        Ok(Status::Other(format!("{command} {arguments}")))
    }
}

fn conhost() -> PathBuf {
    system32().join("conhost.exe")
}

fn system32() -> PathBuf {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    PathBuf::from(root).join("System32")
}

fn check_args(exe: &Path) -> String {
    format!("--headless \"{}\" check", exe.display())
}

fn user_id() -> anyhow::Result<String> {
    let user = std::env::var("USERNAME").context("variavel USERNAME nao definida")?;
    Ok(match std::env::var("USERDOMAIN") {
        Ok(domain) => format!("{domain}\\{user}"),
        Err(_) => user,
    })
}

/// Logon trigger plus a time trigger from now, both repeating every `minutes`; one run
/// at a time, stopped after 10 minutes.
fn task_xml(user: &str, exe: &Path, minutes: u32, now_utc: &str) -> String {
    let user = escape(user);
    let command = escape(&conhost().to_string_lossy());
    let arguments = escape(&check_args(exe));
    let repeat = format!(
        "<Repetition><Interval>PT{minutes}M</Interval><StopAtDurationEnd>false</StopAtDurationEnd></Repetition>"
    );
    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Description>agsync: le o repo de config e avisa mudancas pendentes; nunca aplica</Description>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      <UserId>{user}</UserId>
      {repeat}
    </LogonTrigger>
    <TimeTrigger>
      <Enabled>true</Enabled>
      <StartBoundary>{now_utc}</StartBoundary>
      {repeat}
    </TimeTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <StartWhenAvailable>true</StartWhenAvailable>
    <ExecutionTimeLimit>PT10M</ExecutionTimeLimit>
    <Enabled>true</Enabled>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{command}</Command>
      <Arguments>{arguments}</Arguments>
    </Exec>
  </Actions>
</Task>
"#
    )
}

fn utf16_with_bom(text: &str) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(text.encode_utf16().flat_map(u16::to_le_bytes));
    bytes
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn unescape(text: &str) -> String {
    text.replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Text of the first `<name>` element.
fn tag(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = xml.find(&open)? + open.len();
    let len = xml[start..].find(&close)?;
    Some(unescape(&xml[start..start + len]))
}

fn schtasks(args: &[&str]) -> anyhow::Result<String> {
    let out = Command::new(system32().join("schtasks.exe"))
        .args(args)
        .stdin(Stdio::null())
        .output()
        .context("executar schtasks")?;
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    if !out.status.success() {
        anyhow::bail!(
            "schtasks {} falhou: {}",
            args.first().unwrap_or(&""),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn xml_runs_headless_check_with_limits() {
        let exe = Path::new(r"C:\Program Files\a&b\agsync.exe");
        let xml = task_xml("PC\\ana", exe, 15, "2026-11-13T12:00:00Z");
        assert!(xml.contains("<UserId>PC\\ana</UserId>"));
        assert_eq!(xml.matches("<Interval>PT15M</Interval>").count(), 2);
        assert!(xml.contains("<MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>"));
        assert!(xml.contains("<ExecutionTimeLimit>PT10M</ExecutionTimeLimit>"));
        assert!(xml.contains("<RunLevel>LeastPrivilege</RunLevel>"));
        assert!(xml.contains("<StartBoundary>2026-11-13T12:00:00Z</StartBoundary>"));
        let args = tag(&xml, "Arguments").unwrap();
        assert_eq!(args, check_args(exe));
        assert_eq!(
            args,
            r#"--headless "C:\Program Files\a&b\agsync.exe" check"#
        );
        assert!(
            tag(&xml, "Command")
                .unwrap()
                .ends_with(r"System32\conhost.exe")
        );
    }

    #[test]
    fn utf16_file_starts_with_bom() {
        assert_eq!(utf16_with_bom("<a"), vec![0xFF, 0xFE, b'<', 0, b'a', 0]);
    }
}
