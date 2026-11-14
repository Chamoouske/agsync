//! Windows half of the fake legacy setup: two real Task Scheduler tasks with test
//! names (one running a fake `1mcp serve` that keeps a child `ping` alive), a real user
//! variable with a test name and APM under a temp `LOCALAPPDATA`.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::common::Env;

pub const APM_HINT: &str = "Remove-Item -Recurse -Force '";

/// Real configs this test must never change.
pub fn real_files() -> Vec<PathBuf> {
    let home = agsync::paths::home().unwrap();
    vec![
        home.join(".config").join("opencode").join("opencode.json"),
        home.join(".config").join("opencode").join("opencode.jsonc"),
        home.join(".codex").join("config.toml"),
        home.join(".hermes").join("config.yaml"),
    ]
}

pub fn vars(env: &Env) -> Vec<(String, String)> {
    let local = env.root.path().join("localappdata");
    std::fs::create_dir_all(local.join("Programs").join("apm")).unwrap();
    vec![(
        "LOCALAPPDATA".to_string(),
        local.to_string_lossy().to_string(),
    )]
}

pub struct Fake {
    tasks: [String; 2],
    var: String,
    needle: String,
    /// The serve task really started; a session without an interactive logon (some
    /// CI runners) cannot run it.
    started: bool,
}

fn powershell(script: &str, vars: &[(&str, &str)]) -> String {
    let mut cmd = Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", script]);
    for (key, value) in vars {
        cmd.env(key, value);
    }
    let out = cmd.output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn schtasks(args: &[&str]) -> bool {
    Command::new("schtasks.exe")
        .args(args)
        .output()
        .unwrap()
        .status
        .success()
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn create_task(dir: &Path, name: &str, arguments: &str) {
    let user = format!(
        "{}\\{}",
        std::env::var("USERDOMAIN").unwrap(),
        std::env::var("USERNAME").unwrap()
    );
    let xml = format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <Triggers><TimeTrigger><StartBoundary>2099-01-01T00:00:00</StartBoundary><Enabled>true</Enabled></TimeTrigger></Triggers>
  <Principals><Principal id="Author"><UserId>{}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals>
  <Settings><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><Hidden>true</Hidden><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><ExecutionTimeLimit>PT1H</ExecutionTimeLimit><Enabled>true</Enabled></Settings>
  <Actions Context="Author"><Exec><Command>powershell.exe</Command><Arguments>{}</Arguments></Exec></Actions>
</Task>
"#,
        escape(&user),
        escape(arguments)
    );
    let mut bytes = vec![0xFF, 0xFE];
    bytes.extend(xml.encode_utf16().flat_map(u16::to_le_bytes));
    let file = dir.join(format!("{name}.xml"));
    std::fs::write(&file, bytes).unwrap();
    let file = file.to_string_lossy().to_string();
    assert!(schtasks(&["/create", "/tn", name, "/xml", &file, "/f"]));
}

fn pings(needle: &str) -> usize {
    let script = "@(Get-CimInstance Win32_Process -Filter \"Name='PING.EXE'\" | Where-Object { $_.CommandLine.Contains($env:N) }).Count";
    powershell(script, &[("N", needle)]).parse().unwrap_or(0)
}

fn user_var(name: &str) -> String {
    let script = "[Environment]::GetEnvironmentVariable($env:N, 'User')";
    powershell(script, &[("N", name)])
}

impl Fake {
    pub fn create(env: &Env, name: &str, var: &str) -> Fake {
        let dir = env.root.path().join("legacy");
        std::fs::create_dir_all(&dir).unwrap();
        let count = 600 + std::process::id() % 300;
        let needle = format!("-n {count} 127.0.0.1");
        let fake_1mcp = dir.join("1mcp.cmd");
        std::fs::write(&fake_1mcp, format!("@ping {needle} >nul\r\n")).unwrap();
        let config = dir.join("mcp.json");
        let tasks = [
            format!("{name}-1mcp-serve"),
            format!("{name}-agent-config-update"),
        ];
        let mut fake = Fake {
            tasks: tasks.clone(),
            var: var.to_string(),
            needle,
            started: false,
        };
        let serve = format!(
            "-NoProfile -NonInteractive -WindowStyle Hidden -Command \"& '{}' serve --config '{}'\"",
            fake_1mcp.display(),
            config.display()
        );
        create_task(&dir, &tasks[0], &serve);
        let update = format!("-NoProfile -File \"{}\"", dir.join("update.ps1").display());
        create_task(&dir, &tasks[1], &update);
        let script = "[Environment]::SetEnvironmentVariable($env:N, $env:V, 'User')";
        powershell(script, &[("N", var), ("V", &config.to_string_lossy())]);
        assert!(schtasks(&["/run", "/tn", &tasks[0]]));
        for _ in 0..50 {
            fake.started = pings(&fake.needle) > 0;
            if fake.started {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
        if !fake.started {
            eprintln!("SKIP processo do 1mcp serve: a tarefa nao iniciou nesta sessao");
        }
        fake
    }

    pub fn assert_present(&self) {
        for task in &self.tasks {
            assert!(schtasks(&["/query", "/tn", task]), "task {task} missing");
        }
        if self.started {
            assert_eq!(pings(&self.needle), 1, "fake 1mcp serve not running");
        }
        assert!(!user_var(&self.var).is_empty());
    }

    pub fn assert_removed(&self, backup: &Path) {
        for task in &self.tasks {
            assert!(!schtasks(&["/query", "/tn", task]), "task {task} left");
            let xml = std::fs::read(backup.join(format!("task-{task}.xml"))).unwrap();
            assert!(!xml.is_empty());
        }
        assert_eq!(pings(&self.needle), 0, "child of 1mcp serve still running");
        assert_eq!(user_var(&self.var), "");
        let saved = std::fs::read_to_string(backup.join(format!("env-{}.txt", self.var))).unwrap();
        assert!(saved.starts_with(&format!("{}=", self.var)));
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        for task in &self.tasks {
            schtasks(&["/end", "/tn", task]);
            schtasks(&["/delete", "/tn", task, "/f"]);
        }
        let kill = "Get-CimInstance Win32_Process -Filter \"Name='PING.EXE'\" | Where-Object { $_.CommandLine.Contains($env:N) } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }";
        powershell(kill, &[("N", &self.needle)]);
        let script = "[Environment]::SetEnvironmentVariable($env:N, $null, 'User')";
        powershell(script, &[("N", &self.var)]);
    }
}
