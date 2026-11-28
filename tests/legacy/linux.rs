//! Linux half of the fake legacy setup: runtime user units with test names (a running
//! `1mcp.service` with its drop-in, and the update service and timer), and export lines
//! in the temp `~/.bashrc` and `~/.zshrc`. Without a working `systemctl --user` the
//! units are skipped, with a message.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::common::Env;

pub const APM_HINT: &str = "pip uninstall apm-cli";

/// Real configs this test must never change.
pub fn real_files() -> Vec<PathBuf> {
    let home = agsync_core::paths::home().unwrap();
    vec![
        home.join(".config").join("opencode").join("opencode.json"),
        home.join(".config").join("opencode").join("opencode.jsonc"),
        home.join(".codex").join("config.toml"),
        home.join(".hermes").join("config.yaml"),
        home.join(".bashrc"),
        home.join(".zshrc"),
    ]
}

pub fn vars(_env: &Env) -> Vec<(String, String)> {
    Vec::new()
}

const BASHRC: &str = "# mine\nexport PATH=\"$HOME/bin:$PATH\"\n";

pub struct Fake {
    home: PathBuf,
    var: String,
    units: Option<Units>,
}

struct Units {
    dir: PathBuf,
    proxy: String,
    update: String,
}

fn systemctl(args: &[&str]) -> bool {
    Command::new("systemctl")
        .arg("--user")
        .args(args)
        .output()
        .is_ok_and(|out| out.status.success())
}

fn active(unit: &str) -> bool {
    systemctl(&["is-active", "--quiet", unit])
}

impl Units {
    fn create(name: &str) -> Option<Units> {
        if !systemctl(&["show-environment"]) {
            eprintln!("SKIP units de migrate_legacy: sem `systemctl --user`");
            return None;
        }
        let runtime = std::env::var_os("XDG_RUNTIME_DIR")?;
        let units = Units {
            dir: PathBuf::from(runtime).join("systemd").join("user"),
            proxy: format!("{name}-1mcp"),
            update: format!("{name}-agent-config-update"),
        };
        let dropin = units.dir.join(format!("{}.service.d", units.proxy));
        std::fs::create_dir_all(&dropin).unwrap();
        let write = |file: &str, text: &str| std::fs::write(units.dir.join(file), text).unwrap();
        write(
            &format!("{}.service", units.proxy),
            "[Service]\nExecStart=/bin/sleep 617\n\n[Install]\nWantedBy=default.target\n",
        );
        std::fs::write(
            dropin.join("path.conf"),
            "[Service]\nEnvironment=PATH=/usr/bin\n",
        )
        .unwrap();
        write(
            &format!("{}.service", units.update),
            "[Service]\nType=oneshot\nExecStart=/bin/true\n",
        );
        write(
            &format!("{}.timer", units.update),
            "[Timer]\nOnBootSec=1h\nOnUnitActiveSec=1h\n\n[Install]\nWantedBy=timers.target\n",
        );
        assert!(systemctl(&["daemon-reload"]));
        let proxy = format!("{}.service", units.proxy);
        let timer = format!("{}.timer", units.update);
        assert!(systemctl(&["enable", "--runtime", "--now", &proxy]));
        assert!(systemctl(&["enable", "--runtime", "--now", &timer]));
        Some(units)
    }

    fn files(&self) -> Vec<PathBuf> {
        vec![
            self.dir.join(format!("{}.service", self.proxy)),
            self.dir.join(format!("{}.service.d", self.proxy)),
            self.dir.join(format!("{}.service", self.update)),
            self.dir.join(format!("{}.timer", self.update)),
        ]
    }
}

impl Fake {
    pub fn create(env: &Env, name: &str, var: &str) -> Fake {
        let export = format!("export {var}=/old/agent-config/1mcp/mcp.json\n");
        std::fs::write(env.home.join(".bashrc"), format!("{BASHRC}{export}# end\n")).unwrap();
        std::fs::write(env.home.join(".zshrc"), &export).unwrap();
        let mut fake = Fake {
            home: env.home.clone(),
            var: var.to_string(),
            units: None,
        };
        fake.units = Units::create(name);
        fake
    }

    pub fn assert_present(&self) {
        if let Some(units) = &self.units {
            assert!(active(&format!("{}.service", units.proxy)));
            assert!(active(&format!("{}.timer", units.update)));
        }
        let bashrc = std::fs::read_to_string(self.home.join(".bashrc")).unwrap();
        assert!(bashrc.contains(&format!("export {}=", self.var)));
    }

    pub fn assert_removed(&self, backup: &Path) {
        if let Some(units) = &self.units {
            assert!(!active(&format!("{}.service", units.proxy)));
            assert!(!active(&format!("{}.timer", units.update)));
            for file in units.files() {
                assert!(!file.exists(), "{} left", file.display());
                let saved = backup.join("systemd").join(file.file_name().unwrap());
                assert!(saved.exists(), "{} not backed up", saved.display());
            }
        }
        let bashrc = std::fs::read_to_string(self.home.join(".bashrc")).unwrap();
        assert_eq!(bashrc, format!("{BASHRC}# end\n"));
        assert_eq!(
            std::fs::read_to_string(self.home.join(".zshrc")).unwrap(),
            ""
        );
        let saved = std::fs::read_dir(backup)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with("shell-")
            })
            .count();
        assert_eq!(saved, 2);
    }
}

impl Drop for Fake {
    fn drop(&mut self) {
        let Some(units) = &self.units else {
            return;
        };
        let proxy = format!("{}.service", units.proxy);
        let timer = format!("{}.timer", units.update);
        systemctl(&["disable", "--runtime", "--now", &proxy]);
        systemctl(&["disable", "--runtime", "--now", &timer]);
        for file in units.files() {
            let _ = std::fs::remove_dir_all(&file);
            let _ = std::fs::remove_file(&file);
        }
        systemctl(&["daemon-reload"]);
    }
}
