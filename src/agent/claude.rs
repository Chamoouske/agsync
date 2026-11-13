//! Claude Code adapter.
//!
//! Reads user MCP servers from `.claude.json` (top-level `mcpServers`) and never
//! writes that file: changes go through `claude mcp add-json/remove --scope user`.
//! Skills are folders under `skills/`. Both live in `CLAUDE_CONFIG_DIR` when set,
//! else `~/.claude.json` and `~/.claude/skills`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;

use super::{Adapter, link};
use crate::hash::hash_dir;
use crate::mcp::{McpSpec, hash_value};
use crate::state::LinkMode;

/// Claude Code user-level config.
#[derive(Debug)]
pub struct Claude {
    exe: PathBuf,
    config_file: PathBuf,
    skills_dir: PathBuf,
}

impl Claude {
    /// Finds `claude` in PATH and resolves config locations.
    pub fn detect() -> Result<Claude, String> {
        let exe = crate::which::find("claude").ok_or("claude nao encontrado no PATH")?;
        if let Some(dir) = std::env::var_os("CLAUDE_CONFIG_DIR") {
            return Ok(Claude::at(exe, &PathBuf::from(dir)));
        }
        let home = crate::paths::home().map_err(|e| e.to_string())?;
        Ok(Claude {
            exe,
            config_file: home.join(".claude.json"),
            skills_dir: home.join(".claude").join("skills"),
        })
    }

    /// Adapter for a `CLAUDE_CONFIG_DIR`-style folder.
    pub fn at(exe: PathBuf, config_dir: &Path) -> Claude {
        Claude {
            exe,
            config_file: config_dir.join(".claude.json"),
            skills_dir: config_dir.join("skills"),
        }
    }

    fn servers(&self) -> anyhow::Result<serde_json::Map<String, serde_json::Value>> {
        let text = match std::fs::read_to_string(&self.config_file) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Default::default()),
            Err(e) => {
                return Err(e).with_context(|| format!("ler {}", self.config_file.display()));
            }
        };
        let root: serde_json::Value = serde_json::from_str(&text)
            .with_context(|| format!("{} invalido", self.config_file.display()))?;
        let servers = root.get("mcpServers").and_then(|v| v.as_object());
        Ok(servers.cloned().unwrap_or_default())
    }

    fn has_server(&self, name: &str) -> anyhow::Result<bool> {
        Ok(self.servers()?.contains_key(name))
    }

    /// Runs one `claude` call. Output is only echoed when it cannot hold secrets.
    fn run(&self, args: &[&str], show_output: bool) -> anyhow::Result<()> {
        let out = Command::new(&self.exe)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .with_context(|| format!("executar {}", self.exe.display()))?;
        if out.status.success() {
            return Ok(());
        }
        let code = out.status.code().unwrap_or(-1);
        let what = args.iter().take(2).copied().collect::<Vec<_>>().join(" ");
        if !show_output {
            anyhow::bail!("claude {what} falhou (exit {code})");
        }
        let text = format!(
            "{} {}",
            String::from_utf8_lossy(&out.stdout).trim(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
        anyhow::bail!("claude {what} falhou (exit {code}): {}", text.trim());
    }
}

impl Adapter for Claude {
    fn expands_env_refs(&self) -> bool {
        true
    }

    fn mcps(&self) -> anyhow::Result<BTreeMap<String, String>> {
        let servers = self.servers()?;
        Ok(servers
            .iter()
            .map(|(name, value)| (name.clone(), hash_value(value)))
            .collect())
    }

    fn skills(&self) -> anyhow::Result<BTreeMap<String, String>> {
        let mut found = BTreeMap::new();
        let entries = match std::fs::read_dir(&self.skills_dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(found),
            Err(e) => return Err(e).with_context(|| format!("ler {}", self.skills_dir.display())),
        };
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_dir() {
                let hash = hash_dir(&path).unwrap_or_else(|_| "ilegivel".to_string());
                found.insert(name, hash);
            } else if std::fs::symlink_metadata(&path).is_ok_and(|m| m.is_symlink()) {
                found.insert(name, "link-quebrado".to_string());
            }
        }
        Ok(found)
    }

    fn backup(&self, dir: &Path) -> anyhow::Result<()> {
        if !self.config_file.is_file() {
            return Ok(());
        }
        let millis = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        std::fs::create_dir_all(dir)?;
        let dest = dir.join(format!("claude.json.{millis}"));
        std::fs::copy(&self.config_file, &dest)
            .with_context(|| format!("backup de {}", self.config_file.display()))?;
        Ok(())
    }

    fn write_mcp(&self, name: &str, spec: &McpSpec) -> anyhow::Result<()> {
        if self.has_server(name)? {
            self.run(&["mcp", "remove", "--scope", "user", name], true)?;
        }
        let json = spec.to_json();
        self.run(&["mcp", "add-json", "--scope", "user", name, &json], false)?;
        let written = self.servers()?.get(name).map(hash_value);
        if written.as_deref() != Some(spec.hash().as_str()) {
            anyhow::bail!("claude nao gravou o mcp '{name}' como esperado");
        }
        Ok(())
    }

    fn remove_mcp(&self, name: &str) -> anyhow::Result<()> {
        if !self.has_server(name)? {
            return Ok(());
        }
        self.run(&["mcp", "remove", "--scope", "user", name], true)?;
        if self.has_server(name)? {
            anyhow::bail!("mcp '{name}' continua no arquivo apos remove");
        }
        Ok(())
    }

    fn write_skill(&self, name: &str, cached: &Path, backups: &Path) -> anyhow::Result<LinkMode> {
        link::install(cached, &self.skills_dir.join(name), backups)
    }

    fn remove_skill(&self, name: &str, backups: &Path) -> anyhow::Result<()> {
        link::clear(&self.skills_dir.join(name), backups)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tmp::TempDir;

    #[test]
    fn reads_servers_and_skills_without_cli() {
        let tmp = TempDir::new("agsync-test").unwrap();
        let dir = tmp.path();
        let claude = Claude::at(PathBuf::from("claude-not-called"), dir);
        assert!(claude.mcps().unwrap().is_empty());
        assert!(claude.skills().unwrap().is_empty());

        let spec = McpSpec::Stdio {
            command: "npx".into(),
            args: vec!["-y".into()],
            env: BTreeMap::new(),
        };
        let file = format!(
            r#"{{"userID":"x","mcpServers":{{"echo":{}}},"projects":{{}}}}"#,
            spec.to_json()
        );
        std::fs::write(dir.join(".claude.json"), file).unwrap();
        std::fs::create_dir_all(dir.join("skills").join("hello")).unwrap();
        std::fs::write(dir.join("skills").join("hello").join("SKILL.md"), "x").unwrap();

        assert_eq!(claude.mcps().unwrap()["echo"], spec.hash());
        let skills = claude.skills().unwrap();
        assert_eq!(
            skills["hello"],
            hash_dir(&dir.join("skills").join("hello")).unwrap()
        );

        let backups = dir.join("backups");
        claude.backup(&backups).unwrap();
        assert_eq!(std::fs::read_dir(&backups).unwrap().count(), 1);
    }
}
