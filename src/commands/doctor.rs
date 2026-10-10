use crate::agent;
use crate::board::{self, Board};
use crate::config::Config;
use crate::util::{self, git_ok, GIT_LIMIT};
use anyhow::Result;

struct Report {
    problems: usize,
}

impl Report {
    fn ok(&mut self, what: &str) {
        println!("ok   {what}");
    }
    fn fix(&mut self, what: &str, fix: &str) {
        self.problems += 1;
        println!("fix  {what}: {fix}");
    }
    /// Something to know that nobody should rewrite; it does not fail the check.
    fn warn(&mut self, what: &str, why: &str) {
        println!("warn {what}: {why}");
    }
}

pub fn run() -> Result<i32> {
    let mut r = Report { problems: 0 };
    let path = util::config_path()?;
    let cfg = match Config::load_from(&path) {
        Ok(c) => c,
        Err(e) => {
            r.fix(
                &format!("configuration {}", util::display_path(&path)),
                &format!("{e:#}"),
            );
            return Ok(1);
        }
    };
    r.ok(&format!(
        "configuration {} ({} roles)",
        util::display_path(&path),
        cfg.roles.len()
    ));

    // Trees.
    for role in &cfg.roles {
        if !role.tree.is_dir() {
            let fix = match (&role.worktree_of, &role.branch) {
                (Some(_), Some(_)) => format!("`crew start {} --create` creates the worktree", role.name),
                _ => "create the checkout, or fix `tree` in the configuration".to_string(),
            };
            r.fix(&format!("role {}: tree {} is missing", role.name, role.tree_text), &fix);
        } else if !git_ok(&role.tree, &["rev-parse", "--is-inside-work-tree"]) {
            r.fix(
                &format!("role {}: {} is not a git checkout", role.name, role.tree_text),
                "point `tree` at a checkout or a worktree",
            );
        } else {
            r.ok(&format!("role {}: tree {}", role.name, role.tree_text));
        }
    }

    // Permission classes in the configuration.
    for role in cfg.off_class_roles() {
        r.fix(
            &format!(
                "role {} runs in permission mode {} while the crew runs in {}",
                role.name, role.permission_mode, cfg.agent.permission_mode
            ),
            "messages between the classes are held; give every role the crew's mode",
        );
    }

    // Agent command and live sessions.
    let mut commands: Vec<&str> = cfg.roles.iter().map(|r| r.command.as_str()).collect();
    commands.sort();
    commands.dedup();
    for c in &commands {
        match util::which(c) {
            Some(p) => r.ok(&format!("agent command {c} ({})", util::display_path(&p))),
            None => r.fix(
                &format!("agent command {c} not found"),
                "install it, or set `agent.command` to its path",
            ),
        }
    }
    for c in &commands {
        match agent::list_sessions(c) {
            Ok(sessions) => {
                for role in cfg.roles.iter().filter(|r| r.command == *c) {
                    if let Some(s) = agent::session_in(&sessions, &role.tree) {
                        match super::start::recorded_mode(role) {
                            Some(m) if m == role.permission_mode => {
                                r.ok(&format!("role {}: session {} ({}) started in {m}", role.name, s.name.as_deref().unwrap_or("unnamed"), s.describe()))
                            }
                            Some(m) => r.fix(
                                &format!("role {}: the running session was started in {m}, the configuration now says {}", role.name, role.permission_mode),
                                "started before the configuration changed; restart it with `crew start`",
                            ),
                            None => r.fix(
                                &format!("role {}: a session runs in its tree but was not started by `crew start`", role.name),
                                "its permission class is unknown; restart it with `crew start` so messages are not held",
                            ),
                        }
                    }
                }
                for s in sessions.iter().filter(|s| s.kind == "interactive") {
                    let dir = std::path::Path::new(&s.cwd);
                    if cfg.role_for(dir).is_none() {
                        r.fix(
                            &format!(
                                "session {} runs in {}, which is no role's tree",
                                s.name.as_deref().unwrap_or("unnamed"),
                                util::display_path(dir)
                            ),
                            "add a role for it, or leave it out of the crew knowingly",
                        );
                    }
                }
            }
            Err(e) => r.fix(
                &format!("session listing: {e:#}"),
                "the agent's `agents --json` must work for `status` and `msg`",
            ),
        }
    }

    // Board.
    match Board::new(&cfg) {
        Ok(b) => match b.refresh(GIT_LIMIT) {
            Ok(()) => {
                if b.exists() {
                    r.ok(&format!(
                        "board {} on {} ({} notes)",
                        b.path,
                        b.repo,
                        board::status_lines(&b.read()).len()
                    ));
                    let content = b.read();
                    let mut prev = "";
                    for l in board::status_lines(&content) {
                        if let Some((when, _, _)) = board::parse_note(l) {
                            if when < prev {
                                r.warn(
                                    &format!(
                                        "board: the line {:?} is dated before the one above it",
                                        util::first_words(l, 6)
                                    ),
                                    "a hand-typed time; `crew note` stamps the clock itself, and nobody rewrites another role's line",
                                );
                            }
                            prev = when;
                        }
                    }
                } else {
                    r.fix(
                        &format!("board file {} is missing on {}", b.path, b.repo),
                        "the first `crew note` creates it, or `crew board --sync-roles`",
                    );
                }
            }
            Err(e) => r.fix(
                &format!("board clone: {e:#}"),
                "check `board.repo`, the network and your credentials",
            ),
        },
        Err(e) => r.fix(&format!("board: {e:#}"), "check `board.repo`"),
    }

    // Hook.
    match super::hook::state()? {
        super::hook::HookState::Installed => r.ok(&format!(
            "SessionStart hook in {}",
            util::display_path(&super::hook::settings_path()?)
        )),
        super::hook::HookState::WrongPath(other) => r.fix(
            &format!("SessionStart hook runs `{other}`, not this binary"),
            "run `crew install-hook`",
        ),
        super::hook::HookState::Absent => r.fix(
            "no SessionStart hook",
            "run `crew install-hook` so sessions start with the board",
        ),
    }

    // Terminal, when configured.
    if !cfg.terminal.commands.is_empty() {
        match super::open::find_terminal(&cfg) {
            Some(t) => r.ok(&format!("terminal {t}")),
            None => r.fix(
                &format!("no terminal among {}", cfg.terminal.commands.join(", ")),
                "install one or fix `terminal.commands`",
            ),
        }
    }

    if r.problems == 0 {
        println!("all good");
        Ok(0)
    } else {
        println!("{} thing(s) to fix", r.problems);
        Ok(1)
    }
}
