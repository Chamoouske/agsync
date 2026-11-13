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
    /// Save the config repo and active profiles for this machine.
    Init {
        /// Git url of the config repo.
        url: String,
        /// Branch, tag or full commit SHA to follow (default: main).
        #[arg(long = "ref")]
        git_ref: Option<String>,
        /// Active profile; repeat for several.
        #[arg(long = "profile", required = true)]
        profiles: Vec<String>,
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
    /// Manage secrets.env, the local file for `${VAR}` values.
    Secret {
        #[command(subcommand)]
        command: SecretCmd,
    },
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

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Cmd::Init {
            url,
            git_ref,
            profiles,
        } => agsync::cmd::init(url, git_ref, profiles),
        Cmd::Status => agsync::report::status(),
        Cmd::Diff => agsync::report::diff(),
        Cmd::Apply { yes, adopt } => agsync::apply::apply(yes, adopt),
        Cmd::Secret {
            command: SecretCmd::Set { name },
        } => agsync::cmd::secret_set(&name),
        Cmd::Secret {
            command: SecretCmd::List,
        } => agsync::cmd::secret_list(),
    };
    if let Err(e) = result {
        eprintln!("erro: {e:#}");
        std::process::exit(1);
    }
}
