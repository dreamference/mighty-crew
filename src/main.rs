//! `crew`: a crew of coding-agent sessions on one codebase.

mod activity;
mod agent;
mod chat;
mod commands;
mod config;
mod display;
mod sections;
mod util;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "crew",
    version,
    about = "A crew of coding-agent sessions on one codebase: one role per working tree, a shared chat, messages between roles.",
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
    /// Print the chat's status lines (default: the last 12)
    Chat {
        /// How many lines
        #[arg(short = 'n', long = "lines", default_value_t = 12)]
        n: usize,
        /// All of them
        #[arg(long)]
        all: bool,
        /// Rewrite the chat's role table from the configuration
        #[arg(long)]
        sync_roles: bool,
        /// Print the last fetched copy without fetching
        #[arg(long)]
        cached: bool,
        /// Only the lines written by this role or addressed to it (`to <role>: …`)
        #[arg(long = "for", value_name = "ROLE")]
        for_role: Option<String>,
        /// Only the notes whose text starts with this tag (`decision`, `done`, `blocked`, `heads-up`, `to`)
        #[arg(long, value_name = "TAG", conflicts_with = "decisions")]
        tag: Option<String>,
        /// Only the `decision:` notes
        #[arg(long)]
        decisions: bool,
        /// The decisions still waiting for an answer
        #[arg(long)]
        pending: bool,
    },
    /// Post a decision for the user on the chat (`## Decisions pending`)
    Ask {
        question: String,
        /// The options, comma-separated (repeatable)
        #[arg(long, required = true, value_name = "A,B,C")]
        options: Vec<String>,
        /// The option to recommend
        #[arg(long, value_name = "OPTION")]
        recommend: Option<String>,
        /// Ask as this role
        #[arg(long = "as", value_name = "ROLE")]
        as_role: Option<String>,
    },
    /// Record the choice on a pending decision as a `decision:` note
    Answer {
        id: String,
        option: String,
        /// Answer as this role (default: the tree you are in, else `user`)
        #[arg(long = "as", value_name = "ROLE")]
        as_role: Option<String>,
    },
    /// Say what you are doing right now (no text: show every role's line)
    Now {
        #[arg(trailing_var_arg = true)]
        text: Vec<String>,
        #[arg(long = "as", value_name = "ROLE")]
        as_role: Option<String>,
    },
    /// Record the permission class of a session started by hand, so doctor stops asking for a restart
    Adopt {
        role: String,
        /// The mode it was started in (default: the role's configured mode)
        #[arg(long, value_name = "MODE")]
        mode: Option<String>,
    },
    /// Declare a shared resource and its rules on the chat (no arguments: list them)
    Resource {
        name: Option<String>,
        #[arg(trailing_var_arg = true)]
        rules: Vec<String>,
        #[arg(long = "as", value_name = "ROLE")]
        as_role: Option<String>,
    },
    /// Hand a file or directory to another role through the neutral folder under the cache
    Handoff {
        role: String,
        path: String,
        #[arg(long = "as", value_name = "ROLE")]
        as_role: Option<String>,
    },
    /// From a clone to a live crew: create missing worktrees, open every role (tmux, windows, or the lines to run)
    Up {
        /// Fresh conversations instead of continuing each directory's last one
        #[arg(long)]
        new: bool,
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
    /// Check the configuration, the trees, the chat clone, the hook
    Doctor,
    /// Install the SessionStart hook that prints the chat
    InstallHook,
    /// Remove that hook
    RemoveHook,
    /// Write a configuration from the current repository
    Init {
        /// Overwrite an existing configuration
        #[arg(long)]
        force: bool,
    },
    /// What the SessionStart hook runs: prints the chat when started in a role's tree
    #[command(hide = true)]
    Hook,
}

fn main() {
    let cli = Cli::parse();
    let result = match cli.command {
        Cmd::Start { role, new, create } => commands::start::run(&role, new, create),
        Cmd::Open { role } => commands::open::run(&role),
        Cmd::Chat {
            n,
            all,
            sync_roles,
            cached,
            for_role,
            tag,
            decisions,
            pending,
        } => {
            let tag = if decisions { Some("decision".to_string()) } else { tag };
            commands::chat::run(n, all, sync_roles, cached, pending, for_role.as_deref(), tag.as_deref())
        }
        Cmd::Ask {
            question,
            options,
            recommend,
            as_role,
        } => commands::ask::run(&question, &options, recommend.as_deref(), as_role.as_deref()),
        Cmd::Answer { id, option, as_role } => commands::answer::run(&id, &option, as_role.as_deref()),
        Cmd::Now { text, as_role } => {
            let joined = text.join(" ");
            commands::now::run(
                if text.is_empty() { None } else { Some(joined.as_str()) },
                as_role.as_deref(),
            )
        }
        Cmd::Adopt { role, mode } => commands::adopt::run(&role, mode.as_deref()),
        Cmd::Resource { name, rules, as_role } => {
            commands::resource::run(name.as_deref(), &rules.join(" "), as_role.as_deref())
        }
        Cmd::Handoff { role, path, as_role } => commands::handoff::run(&role, &path, as_role.as_deref()),
        Cmd::Up { new } => commands::up::run(new),
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
