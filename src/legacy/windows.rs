//! Windows: the `1mcp-serve` and `agent-config-update` tasks, the user variable
//! `ONE_MCP_CONFIG` and APM under `%LOCALAPPDATA%\Programs\apm`.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::Context;

use windows_sys::Wdk::System::Threading::NtQueryInformationProcess;
use windows_sys::Win32::Foundation::{CloseHandle, INVALID_HANDLE_VALUE, UNICODE_STRING};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    HWND_BROADCAST, SMTO_ABORTIFHUNG, SendMessageTimeoutW, WM_SETTINGCHANGE,
};
use winreg::RegKey;
use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};

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
        delete_user_var(&self.name)?;
        Ok(())
    }
}

/// Value of a user environment variable, read from the registry, not this process.
fn user_var(name: &str) -> anyhow::Result<Option<String>> {
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey("Environment")
        .context(r"abrir HKCU\Environment")?;
    match key.get_value::<String, _>(name) {
        Ok(value) => Ok(Some(value)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e).with_context(|| format!(r"ler {name} em HKCU\Environment")),
    }
}

/// Deletes a user environment variable from the registry and tells running programs.
fn delete_user_var(name: &str) -> anyhow::Result<()> {
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags("Environment", KEY_SET_VALUE)
        .context(r"abrir HKCU\Environment")?;
    match key.delete_value(name) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(e).with_context(|| format!(r"apagar {name} de HKCU\Environment")),
    }
    broadcast_environment_change();
    Ok(())
}

fn broadcast_environment_change() {
    let area: Vec<u16> = "Environment\0".encode_utf16().collect();
    let mut result = 0usize;
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            area.as_ptr() as isize,
            SMTO_ABORTIFHUNG,
            5000,
            &mut result,
        );
    }
}

/// Kills, with their children, the processes whose command line holds `needle`.
fn kill_trees(needle: &str) -> anyhow::Result<()> {
    let me = std::process::id();
    for pid in process_ids()? {
        if pid != me && command_line(pid).is_some_and(|line| line.contains(needle)) {
            let _ = Command::new(system32().join("taskkill.exe"))
                .args(["/PID", &pid.to_string(), "/T", "/F"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
        }
    }
    Ok(())
}

fn process_ids() -> anyhow::Result<Vec<u32>> {
    let mut pids = Vec::new();
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot == INVALID_HANDLE_VALUE {
            anyhow::bail!("listar processos: {}", std::io::Error::last_os_error());
        }
        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
        let mut more = Process32FirstW(snapshot, &mut entry) != 0;
        while more {
            pids.push(entry.th32ProcessID);
            more = Process32NextW(snapshot, &mut entry) != 0;
        }
        CloseHandle(snapshot);
    }
    Ok(pids)
}

/// Command line of a process, or `None` when it is gone or not ours to open.
fn command_line(pid: u32) -> Option<String> {
    const PROCESS_COMMAND_LINE_INFORMATION: i32 = 60;
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return None;
        }
        let mut buffer = vec![0u64; 512];
        let line = loop {
            let mut needed = 0u32;
            let status = NtQueryInformationProcess(
                handle,
                PROCESS_COMMAND_LINE_INFORMATION,
                buffer.as_mut_ptr().cast(),
                (buffer.len() * 8) as u32,
                &mut needed,
            );
            if status >= 0 {
                let text = &*(buffer.as_ptr() as *const UNICODE_STRING);
                let units = std::slice::from_raw_parts(text.Buffer, text.Length as usize / 2);
                break Some(String::from_utf16_lossy(units));
            }
            if (needed as usize) <= buffer.len() * 8 {
                break None;
            }
            buffer = vec![0u64; (needed as usize).div_ceil(8)];
        };
        CloseHandle(handle);
        line
    }
}

fn system32() -> PathBuf {
    let root = std::env::var_os("SystemRoot").unwrap_or_else(|| r"C:\Windows".into());
    PathBuf::from(root).join("System32")
}
