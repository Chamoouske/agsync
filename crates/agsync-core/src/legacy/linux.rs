//! Linux: the `1mcp.service` (with its `1mcp.service.d/` drop-in) and
//! `agent-config-update.service`/`.timer` user units, `export ONE_MCP_CONFIG=` lines
//! in `~/.bashrc` and `~/.zshrc`, and APM installed by script or pip.

use std::path::{Path, PathBuf};

use anyhow::Context;

use super::{Leftover, System, test_name, unit_name, var_name};
use crate::schedule::systemd::systemctl;

pub struct Linux;

impl System for Linux {
    fn leftovers(&self) -> Vec<Box<dyn Leftover>> {
        let mut found: Vec<Box<dyn Leftover>> = Vec::new();
        match unit_dir() {
            Ok(dir) => found.extend(units(&dir)),
            Err(e) => eprintln!("aviso: pasta das units de usuario: {e:#}"),
        }
        if let Ok(home) = crate::paths::home() {
            let var = var_name();
            for file in [home.join(".bashrc"), home.join(".zshrc")] {
                if has_export(&file, &var) {
                    found.push(Box::new(ExportLine {
                        file,
                        var: var.clone(),
                    }));
                }
            }
        }
        found
    }

    fn apm_uninstall(&self) -> Option<String> {
        let exe = crate::which::find("apm")?;
        let head = std::fs::read(&exe).ok()?;
        let python = head.starts_with(b"#!") && String::from_utf8_lossy(&head).contains("python");
        let text = exe.to_string_lossy();
        let command = match (python, text.contains("pipx")) {
            (true, true) => "pipx uninstall apm-cli".to_string(),
            (true, false) => "pip uninstall apm-cli".to_string(),
            (false, _) => format!("rm '{text}'  (APM instalado por script)"),
        };
        Some(command)
    }
}

/// `~/.config/systemd/user`, or the runtime dir under a test name.
fn unit_dir() -> anyhow::Result<PathBuf> {
    if test_name().is_some() {
        let dir = std::env::var_os("XDG_RUNTIME_DIR").context("XDG_RUNTIME_DIR nao definida")?;
        return Ok(PathBuf::from(dir).join("systemd").join("user"));
    }
    let config = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(dir) => PathBuf::from(dir),
        None => crate::paths::home()?.join(".config"),
    };
    Ok(config.join("systemd").join("user"))
}

/// The two unit groups, each when one of its files exists.
fn units(dir: &Path) -> Vec<Box<dyn Leftover>> {
    let proxy = unit_name("1mcp");
    let update = unit_name("agent-config-update");
    let groups = [
        Units {
            units: vec![format!("{proxy}.service")],
            files: vec![
                dir.join(format!("{proxy}.service")),
                dir.join(format!("{proxy}.service.d")),
            ],
        },
        Units {
            units: vec![format!("{update}.timer"), format!("{update}.service")],
            files: vec![
                dir.join(format!("{update}.timer")),
                dir.join(format!("{update}.service")),
            ],
        },
    ];
    groups
        .into_iter()
        .filter(|group| group.files.iter().any(|f| f.exists()))
        .map(|group| Box::new(group) as Box<dyn Leftover>)
        .collect()
}

struct Units {
    units: Vec<String>,
    files: Vec<PathBuf>,
}

impl Leftover for Units {
    fn category(&self) -> &'static str {
        "units systemd de usuario"
    }

    fn describe(&self) -> String {
        self.units.join(" + ")
    }

    /// `disable --now`, files copied to the backup and deleted, then `daemon-reload`.
    fn remove(&self, backup: &Path) -> anyhow::Result<()> {
        for unit in &self.units {
            let mut args = vec!["disable"];
            if test_name().is_some() {
                args.push("--runtime");
            }
            args.extend(["--now", unit.as_str()]);
            if let Err(e) = systemctl(&args) {
                eprintln!("aviso: {e:#}");
            }
        }
        let dest = backup.join("systemd");
        for file in self.files.iter().filter(|f| f.exists()) {
            let name = file.file_name().unwrap_or_default();
            if file.is_dir() {
                crate::agent::link::copy_dir(file, &dest.join(name))?;
                std::fs::remove_dir_all(file)
            } else {
                std::fs::create_dir_all(&dest)?;
                std::fs::copy(file, dest.join(name))?;
                std::fs::remove_file(file)
            }
            .with_context(|| format!("apagar {}", file.display()))?;
        }
        systemctl(&["daemon-reload"])
    }
}

struct ExportLine {
    file: PathBuf,
    var: String,
}

impl Leftover for ExportLine {
    fn category(&self) -> &'static str {
        "variavel ONE_MCP_CONFIG nos perfis de shell"
    }

    fn describe(&self) -> String {
        format!("export {}= em {}", self.var, self.file.display())
    }

    fn remove(&self, backup: &Path) -> anyhow::Result<()> {
        let text = std::fs::read_to_string(&self.file)
            .with_context(|| format!("ler {}", self.file.display()))?;
        let name = self.file.file_name().unwrap_or_default().to_string_lossy();
        crate::agent::backup::copy(&self.file, backup, &format!("shell-{name}"))?;
        crate::atomic::write_atomic(&self.file, without_export(&text, &self.var).as_bytes())
    }
}

fn is_export(line: &str, var: &str) -> bool {
    let rest = line.trim_start().strip_prefix("export ");
    rest.is_some_and(|rest| rest.trim_start().starts_with(&format!("{var}=")))
}

fn has_export(file: &Path, var: &str) -> bool {
    let text = std::fs::read_to_string(file).unwrap_or_default();
    text.lines().any(|line| is_export(line, var))
}

/// `text` without its `export <var>=` lines; every other byte kept.
fn without_export(text: &str, var: &str) -> String {
    text.split_inclusive('\n')
        .filter(|line| !is_export(line, var))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_export_line_goes() {
        let text = "# mine\nexport ONE_MCP_CONFIG=/x/mcp.json\n  export ONE_MCP_CONFIG_OTHER=1\nexport PATH=/a\n";
        assert_eq!(
            without_export(text, "ONE_MCP_CONFIG"),
            "# mine\n  export ONE_MCP_CONFIG_OTHER=1\nexport PATH=/a\n"
        );
        assert_eq!(
            without_export("export ONE_MCP_CONFIG=1", "ONE_MCP_CONFIG"),
            ""
        );
        assert!(is_export(
            "  export  ONE_MCP_CONFIG=\"a b\"",
            "ONE_MCP_CONFIG"
        ));
    }
}
