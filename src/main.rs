//! agsync command line.

use agsync_core::prompt::from_flag;
use clap::{Parser, Subcommand};

mod cmd;

/// Sync MCP servers and skills from a git config repo into coding agents.
#[derive(Parser)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Set up this machine: machine.toml, repo access, schedule, shell hook, first apply.
    Init {
        /// Git url of the config repo; asked on the terminal when omitted.
        url: Option<String>,
        /// Branch, tag or full commit SHA to follow (default: main).
        #[arg(long = "ref")]
        git_ref: Option<String>,
        /// Active profile; repeat for several. Asked on the terminal when omitted.
        #[arg(long = "profile")]
        profiles: Vec<String>,
        /// Install the shell hook and apply without asking.
        #[arg(long)]
        yes: bool,
        /// Apply pending updates without asking, as `agsync apply --yes` does; the shell
        /// hook question stays. What was applied, skipped or failed is still printed.
        /// `silentUpdate: true` in agsync.yaml does the same.
        #[arg(long)]
        silent_update: bool,
    },
    /// Show applied and remote commits, pending changes, drift, conflicts and skips.
    Status,
    /// Show what the next apply would change, per agent.
    Diff,
    /// Apply the config repo to every agent found on this machine.
    Apply {
        /// Apply without asking; required when stdin is not a terminal.
        #[arg(long)]
        yes: bool,
        /// Replace manual installs (MCP, skill, plugin) whose name, kind and agent match an
        /// item of the config repo with its definition, and track them from now on.
        #[arg(long)]
        adopt: bool,
    },
    /// Apply again the commit applied before the current one, through the normal plan.
    Rollback {
        /// Apply without asking; required when stdin is not a terminal.
        #[arg(long)]
        yes: bool,
    },
    /// Read the remote only; record pending changes and notify once. Run by the scheduler.
    Check,
    /// Print the pending line; no network. Started by the shell profile.
    Hook {
        #[command(subcommand)]
        command: Option<HookCmd>,
    },
    /// Check git, the config repo, agents, requires, secrets and source repos.
    Doctor,
    /// Register, remove or show the periodic `agsync check`.
    Schedule {
        #[command(subcommand)]
        command: ScheduleCmd,
    },
    /// Back up and remove what APM, 1MCP and the agent-config scripts left here.
    MigrateLegacy {
        /// Remove without asking; required when stdin is not a terminal.
        #[arg(long)]
        yes: bool,
    },
    /// Remove every agsync entry from the agents, the schedule, the shell hook and the state.
    Uninstall {
        /// Uninstall without asking; required when stdin is not a terminal.
        #[arg(long)]
        yes: bool,
        /// Also delete the config and data dirs: secrets.env, cache and every backup.
        #[arg(long)]
        purge: bool,
    },
    /// Replace this binary with the latest release (installed by Scoop: tells to run `scoop update agsync`).
    Update {
        /// Update without asking; required when stdin is not a terminal.
        #[arg(long)]
        yes: bool,
    },
    /// Manage secrets.env, the local file for `${VAR}` values.
    Secret {
        #[command(subcommand)]
        command: SecretCmd,
    },
}

#[derive(Subcommand)]
enum HookCmd {
    /// Add `agsync hook` to the profile of each shell on this machine.
    Install {
        /// Install without asking; required when stdin is not a terminal.
        #[arg(long)]
        yes: bool,
    },
    /// Remove the agsync block from every shell profile.
    Remove,
}

#[derive(Subcommand)]
enum ScheduleCmd {
    /// Run `agsync check` every `check_interval_minutes` (Task Scheduler, systemd or cron).
    Install,
    /// Delete the periodic check.
    Remove,
    /// Show whether the check is registered and runs this binary.
    Status,
}

#[derive(Subcommand)]
enum SecretCmd {
    /// Store a value; read without echo from the terminal, or from stdin.
    Set {
        /// Variable name, as used in `${NAME}`.
        name: String,
    },
    /// Show the names defined in secrets.env (never the values).
    List,
}

impl Cmd {
    /// Commands that print the pending line on stderr first: all but the ones that
    /// handle `pending.json` or `update.json` themselves.
    fn warns_pending(&self) -> bool {
        !matches!(
            self,
            Cmd::Hook { .. } | Cmd::Check | Cmd::Apply { .. } | Cmd::Update { .. }
        )
    }
}

fn main() {
    let cli = Cli::parse();
    if cli.command.warns_pending() {
        agsync_core::hook::warn_pending();
    }
    let result = match cli.command {
        Cmd::Init {
            url,
            git_ref,
            profiles,
            yes,
            silent_update,
        } => agsync_core::init::init(url, git_ref, profiles, yes, silent_update),
        Cmd::Status => agsync_core::report::status().map(|()| true),
        Cmd::Diff => agsync_core::report::diff().map(|()| true),
        Cmd::Apply { yes, adopt } => {
            agsync_core::apply::apply(from_flag(yes), adopt).map(|()| true)
        }
        Cmd::Rollback { yes } => agsync_core::rollback::rollback(from_flag(yes)).map(|()| true),
        Cmd::Check => agsync_core::check::check().map(|()| true),
        Cmd::Hook { command } => match command {
            None => agsync_core::hook::hook().map(|()| true),
            Some(HookCmd::Install { yes }) => agsync_core::shell::install(from_flag(yes)),
            Some(HookCmd::Remove) => agsync_core::shell::remove().map(|()| true),
        },
        Cmd::Doctor => agsync_core::doctor::doctor(),
        Cmd::Schedule { command } => match command {
            ScheduleCmd::Install => agsync_core::schedule::install().map(|()| true),
            ScheduleCmd::Remove => agsync_core::schedule::remove().map(|()| true),
            ScheduleCmd::Status => agsync_core::schedule::status(),
        },
        Cmd::MigrateLegacy { yes } => agsync_core::legacy::migrate_legacy(from_flag(yes)),
        Cmd::Uninstall { yes, purge } => agsync_core::uninstall::uninstall(from_flag(yes), purge),
        Cmd::Update { yes } => agsync_core::self_update::update(from_flag(yes)),
        Cmd::Secret {
            command: SecretCmd::Set { name },
        } => cmd::secret_set(&name).map(|()| true),
        Cmd::Secret {
            command: SecretCmd::List,
        } => cmd::secret_list().map(|()| true),
    };
    match result {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(e) => {
            eprintln!("erro: {e:#}");
            std::process::exit(1);
        }
    }
}
