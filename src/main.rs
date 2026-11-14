//! agsync command line.

use clap::{Parser, Subcommand};

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
        /// Git url of the config repo.
        url: String,
        /// Branch, tag or full commit SHA to follow (default: main).
        #[arg(long = "ref")]
        git_ref: Option<String>,
        /// Active profile; repeat for several.
        #[arg(long = "profile", required = true)]
        profiles: Vec<String>,
        /// Install the shell hook and apply without asking.
        #[arg(long)]
        yes: bool,
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
        /// Take over manual entries whose name collides with the config repo.
        #[arg(long)]
        adopt: bool,
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
    /// handle `pending.json` themselves.
    fn warns_pending(&self) -> bool {
        !matches!(self, Cmd::Hook { .. } | Cmd::Check | Cmd::Apply { .. })
    }
}

fn main() {
    let cli = Cli::parse();
    if cli.command.warns_pending() {
        agsync::hook::warn_pending();
    }
    let result = match cli.command {
        Cmd::Init {
            url,
            git_ref,
            profiles,
            yes,
        } => agsync::init::init(url, git_ref, profiles, yes),
        Cmd::Status => agsync::report::status().map(|()| true),
        Cmd::Diff => agsync::report::diff().map(|()| true),
        Cmd::Apply { yes, adopt } => agsync::apply::apply(yes, adopt).map(|()| true),
        Cmd::Check => agsync::check::check().map(|()| true),
        Cmd::Hook { command } => match command {
            None => agsync::hook::hook().map(|()| true),
            Some(HookCmd::Install { yes }) => agsync::shell::install(yes),
            Some(HookCmd::Remove) => agsync::shell::remove().map(|()| true),
        },
        Cmd::Doctor => agsync::doctor::doctor(),
        Cmd::Schedule { command } => match command {
            ScheduleCmd::Install => agsync::schedule::install().map(|()| true),
            ScheduleCmd::Remove => agsync::schedule::remove().map(|()| true),
            ScheduleCmd::Status => agsync::schedule::status(),
        },
        Cmd::Secret {
            command: SecretCmd::Set { name },
        } => agsync::cmd::secret_set(&name).map(|()| true),
        Cmd::Secret {
            command: SecretCmd::List,
        } => agsync::cmd::secret_list().map(|()| true),
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
