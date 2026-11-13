//! `init`, `status` and `diff` commands.

use crate::git;
use crate::machine::Machine;
use crate::paths::Dirs;

/// Writes `machine.toml` after checking the repo answers `git ls-remote`.
pub fn init(url: String, git_ref: Option<String>, profiles: Vec<String>) -> anyhow::Result<()> {
    let dirs = Dirs::from_env()?;
    let machine = Machine::new(url, git_ref, profiles);
    let commit = git::resolve(&machine.repo, &machine.git_ref)?;
    let file = dirs.machine_file();
    machine.save(&file)?;
    println!(
        "repo ok: {} ({} em {})",
        machine.repo, machine.git_ref, commit
    );
    println!("perfis ativos: {}", machine.profiles.join(", "));
    println!("gravado {}", file.display());
    Ok(())
}
