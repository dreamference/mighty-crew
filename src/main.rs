//! `crew`: a crew of coding-agent sessions on one codebase.

mod agent;
mod board;
mod commands;
mod config;
mod util;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "crew",
    version,
    about = "A crew of coding-agent sessions on one codebase: one role per working tree, a shared board, messages between roles.",
    after_help = "Configuration: ~/.config/mighty-crew/crew.toml (MIGHTY_CREW_CONFIG overrides). Every command exits 1 on failure with one line saying why."
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Start (or resume) the role's agent in its tree
    Start {
        role: String,
        /// Start a fresh conversation instead of continuing the directory's last one
        #[arg(long)]
        new: bool,
        /// Create the role's worktree when it is missing and configured (`worktree_of`, `branch`)
        #[arg(long)]
        create: bool,
    },
    /// The same, in a new terminal window (needs a display)
    Open { role: String },
    /// Print the board's status lines (default: the last 12)
    Board {
        /// How many lines
        #[arg(short = 'n', long = "lines", default_value_t = 12)]
        n: usize,
        /// All of them
        #[arg(long)]
        all: bool,
        /// Rewrite the board's role table from the configuration
        #[arg(long)]
        sync_roles: bool,
        /// Print the last fetched copy without fetching
        #[arg(long)]
        cached: bool,
        /// Only the lines written by this role or addressed to it (`to <role>: …`)
        #[arg(long = "for", value_name = "ROLE")]
        for_role: Option<String>,
    },
    /// Append a dated line as the role whose tree you are in
    Note {
        /// Write as this role instead of the one owning the current directory
        #[arg(long = "as", value_name = "ROLE")]
        as_role: Option<String>,
        #[arg(required = true, trailing_var_arg = true)]
        text: Vec<String>,
    },
    /// Send a line to that role's running session
    Msg {
        role: String,
        #[arg(required = true, trailing_var_arg = true)]
        text: Vec<String>,
    },
    /// Roles, trees, branches, heads, running or not, last note
    Status,
    /// Check the configuration, the trees, the board clone, the hook
    Doctor,
    /// Install the SessionStart hook that prints the board
    InstallHook,
    /// Remove that hook
    RemoveHook,
    /// Write a configuration from the current repository
    Init {
        /// Overwrite an existing configuration
        #[arg(long)]
        force: bool,
    },
    /// What the SessionStart hook runs: prints the board when started in a role's tree
    #[command(hide = true)]
    Hook,
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Cmd::Start { role, new, create } => commands::start::run(&role, new, create),
        Cmd::Open { role } => commands::open::run(&role),
        Cmd::Board {
            n,
            all,
            sync_roles,
            cached,
            for_role,
        } => commands::board::run(n, all, sync_roles, cached, for_role.as_deref()),
        Cmd::Note { as_role, text } => commands::note::run(as_role.as_deref(), &text.join(" ")),
        Cmd::Msg { role, text } => commands::msg::run(&role, &text.join(" ")),
        Cmd::Status => commands::status::run(),
        Cmd::Doctor => commands::doctor::run(),
        Cmd::InstallHook => commands::hook::install(),
        Cmd::RemoveHook => commands::hook::remove(),
        Cmd::Init { force } => commands::init::run(force),
        Cmd::Hook => commands::hook::session_start(),
    };
    match result {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("crew: {e:#}");
            std::process::exit(1);
        }
    }
}
