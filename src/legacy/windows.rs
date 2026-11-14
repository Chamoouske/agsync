//! Windows: the `1mcp-serve` and `agent-config-update` tasks, the user variable
//! `ONE_MCP_CONFIG` and APM under `%LOCALAPPDATA%\Programs\apm`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::Context;

use super::{Leftover, System, unit_name, var_name};
use crate::schedule::Scheduler;
use crate::schedule::task::TaskScheduler;

pub struct Windows;

impl System for Windows {
    fn leftovers(&self) -> Vec<Box<dyn Leftover>> {
        let mut found: Vec<Box<dyn Leftover>> = Vec::new();
        for base in ["1mcp-serve", "agent-config-update"] {
            let name = unit_name(base);
            let task = TaskScheduler::new(&name);
            if task.query_xml().is_some() {
                found.push(Box::new(Task { name, task }));
            }
        }
        let name = var_name();
        match user_var(&name) {
            Ok(Some(value)) => found.push(Box::new(UserVar { name, value })),
            Ok(None) => {}
            Err(e) => eprintln!("aviso: ler a variavel de usuario {name} falhou: {e:#}"),
        }
        found
    }

    fn apm_uninstall(&self) -> Option<String> {
        let dir = PathBuf::from(std::env::var_os("LOCALAPPDATA")?)
            .join("Programs")
            .join("apm");
        dir.is_dir().then(|| {
            format!(
                "Remove-Item -Recurse -Force '{}'  (e tire essa pasta do PATH do usuario)",
                dir.display()
            )
        })
    }
}

struct Task {
    name: String,
    task: TaskScheduler,
}

impl Leftover for Task {
    fn category(&self) -> &'static str {
        "tarefas agendadas"
    }

    fn describe(&self) -> String {
        format!("tarefa {}", self.name)
    }

    /// Saves the XML, kills the processes started by its action (`schtasks /end`
    /// alone leaves the children of `1mcp serve` running), ends it and deletes it.
    fn remove(&self, backup: &Path) -> anyhow::Result<()> {
        let xml = self.task.query_xml().unwrap_or_default();
        let file = backup.join(format!("task-{}.xml", self.name));
        std::fs::write(&file, xml).with_context(|| format!("gravar {}", file.display()))?;
        if let Some((_, arguments)) = self.task.action()
            && !arguments.trim().is_empty()
        {
            kill_trees(&arguments)?;
        }
        let _ = self.task.end();
        self.task.remove()?;
        Ok(())
    }
}

struct UserVar {
    name: String,
    value: String,
}

impl Leftover for UserVar {
    fn category(&self) -> &'static str {
        "variavel de ambiente de usuario"
    }

    fn describe(&self) -> String {
        format!("{}={}", self.name, self.value)
    }

    fn remove(&self, backup: &Path) -> anyhow::Result<()> {
        let file = backup.join(format!("env-{}.txt", self.name));
        std::fs::write(&file, format!("{}={}\n", self.name, self.value))
            .with_context(|| format!("gravar {}", file.display()))?;
        powershell(
            "[Environment]::SetEnvironmentVariable($env:AGSYNC_VAR, $null, 'User')",
            &[("AGSYNC_VAR", &self.name)],
        )?;
        Ok(())
    }
}

/// Value of a user environment variable, read from the registry, not this process.
fn user_var(name: &str) -> anyhow::Result<Option<String>> {
    let script = "$v = [Environment]::GetEnvironmentVariable($env:AGSYNC_VAR, 'User'); if ($null -ne $v) { 'set:' + $v }";
    let out = powershell(script, &[("AGSYNC_VAR", name)])?;
    Ok(out.trim_end().strip_prefix("set:").map(str::to_string))
}

/// Kills, with their children, the processes whose command line holds `needle`.
fn kill_trees(needle: &str) -> anyhow::Result<()> {
    let script = "Get-CimInstance Win32_Process | Where-Object { $_.CommandLine -and $_.CommandLine.Contains($env:AGSYNC_NEEDLE) -and $_.ProcessId -ne $PID } | ForEach-Object { $_.ProcessId }";
    let out = powershell(script, &[("AGSYNC_NEEDLE", needle)])?;
    for pid in out.split_whitespace() {
        let _ = Command::new(system32().join("taskkill.exe"))
            .args(["/PID", pid, "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    Ok(())
}

/// Runs a Windows PowerShell script; values go through the environment, never quoted
/// into the script.
fn powershell(script: &str, vars: &[(&str, &str)]) -> anyhow::Result<String> {
    let exe = system32()
        .join("WindowsPowerShell")
        .join("v1.0")
        .join("powershell.exe");
    let mut cmd = Command::new(exe);
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", script])
        .stdin(Stdio::null());
    for (key, value) in vars {
        cmd.env(key, value);
    }
    let out = cmd.output().context("executar powershell")?;
    if !out.status.success() {
        anyhow::bail!(
            "powershell falhou: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn system32() -> PathBuf {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    PathBuf::from(root).join("System32")
}
