//! `agsync migrate-legacy`: finds what the old setup (APM, 1MCP and the agent-config
//! scripts) left on this machine, backs it up and removes it, item by item.
//!
//! What differs by OS (scheduled tasks or systemd units, where `ONE_MCP_CONFIG` lives,
//! how APM was installed) sits behind [`System`]. Every removable thing is a
//! [`Leftover`]. Skills and the npm and APM installs are only reported.

use std::path::Path;

use crate::paths::Dirs;

mod agents;
mod apm;
#[cfg(not(windows))]
mod linux;
mod skills;
#[cfg(windows)]
mod windows;

/// One thing the old setup left, removable after a backup.
pub trait Leftover {
    /// Heading it is listed under.
    fn category(&self) -> &'static str;
    /// One line naming it.
    fn describe(&self) -> String;
    /// Copies what it needs into `backup`, then removes it.
    fn remove(&self, backup: &Path) -> anyhow::Result<()>;
}

/// The parts of the old setup that differ by OS.
pub trait System {
    /// Scheduled tasks or units, and the `ONE_MCP_CONFIG` variable.
    fn leftovers(&self) -> Vec<Box<dyn Leftover>>;
    /// Command that uninstalls APM as it was installed here, when found.
    fn apm_uninstall(&self) -> Option<String>;
}

/// When set to `agsync-test-<anything>`, legacy tasks and units are looked up as
/// `<that>-<name>` (on Linux in the runtime dir), so tests never meet the real ones.
pub const TEST_NAME_VAR: &str = "AGSYNC_TEST_LEGACY_NAME";

/// When set to `AGSYNC_TEST_<anything>`, that variable stands for `ONE_MCP_CONFIG`.
pub const TEST_VAR_VAR: &str = "AGSYNC_TEST_LEGACY_VAR";

/// The test name from [`TEST_NAME_VAR`], when it has the test prefix.
pub fn test_name() -> Option<String> {
    let name = std::env::var(TEST_NAME_VAR).ok()?;
    name.starts_with("agsync-test-").then_some(name)
}

/// Name of a legacy task or unit: `base`, or `<test name>-<base>` in tests.
pub fn unit_name(base: &str) -> String {
    match test_name() {
        Some(prefix) => format!("{prefix}-{base}"),
        None => base.to_string(),
    }
}

/// `ONE_MCP_CONFIG`, or the test variable from [`TEST_VAR_VAR`].
pub fn var_name() -> String {
    let test = std::env::var(TEST_VAR_VAR).ok();
    test.filter(|name| name.starts_with("AGSYNC_TEST_"))
        .unwrap_or_else(|| "ONE_MCP_CONFIG".to_string())
}

#[cfg(windows)]
fn system() -> Box<dyn System> {
    Box::new(windows::Windows)
}

#[cfg(not(windows))]
fn system() -> Box<dyn System> {
    Box::new(linux::Linux)
}

/// Lists, asks, backs up and removes. False when some item failed.
pub fn migrate_legacy(yes: bool) -> anyhow::Result<bool> {
    let dirs = Dirs::from_env()?;
    let home = crate::paths::home()?;
    let system = system();
    let mut found = system.leftovers();
    found.extend(agents::leftovers());
    found.extend(apm::leftover(&home));
    print_found(&found);
    skills::print(&home, &dirs);
    let commands = manual_commands(system.as_ref());

    if found.is_empty() {
        println!("nada do sistema antigo para remover");
        print_commands(&commands);
        return Ok(true);
    }
    let question = "Fazer backup e remover estes itens?";
    if !crate::prompt::confirm(yes, question, "agsync migrate-legacy")? {
        return Ok(true);
    }
    crate::secret::file::private_dir(&dirs.backups())?;
    let stamp = crate::checklog::utc_now().replace(':', "-");
    let backup = dirs.backups().join(format!("legacy-{stamp}"));
    std::fs::create_dir_all(&backup)?;
    let mut failures = 0;
    for item in &found {
        match item.remove(&backup) {
            Ok(()) => println!("removido: {}", item.describe()),
            Err(e) => {
                failures += 1;
                eprintln!("falhou: {}: {e:#}", item.describe());
            }
        }
    }
    println!("backup em {}", backup.display());
    print_commands(&commands);
    if failures > 0 {
        eprintln!("{failures} item(ns) falharam; os demais foram removidos");
    }
    Ok(failures == 0)
}

fn print_found(found: &[Box<dyn Leftover>]) {
    let mut category = "";
    for item in found {
        if item.category() != category {
            category = item.category();
            println!("{category}:");
        }
        println!("  {}", item.describe());
    }
}

/// What agsync leaves to the user: npm and APM uninstalls.
fn manual_commands(system: &dyn System) -> Vec<String> {
    let mut commands = Vec::new();
    if crate::which::find("1mcp").is_some() {
        commands.push("npm uninstall -g @1mcp/agent".to_string());
    }
    commands.extend(system.apm_uninstall());
    commands
}

fn print_commands(commands: &[String]) {
    if commands.is_empty() {
        return;
    }
    println!("o agsync nao desinstala programas; rode voce mesmo:");
    for command in commands {
        println!("  {command}");
    }
}
