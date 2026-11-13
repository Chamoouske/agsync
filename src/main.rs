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
        /// Branch or tag to follow (default: main).
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
    };
    if let Err(e) = result {
        eprintln!("erro: {e:#}");
        std::process::exit(1);
    }
}
