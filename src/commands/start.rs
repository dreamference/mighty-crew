use crate::agent;
use crate::board::{self, Board};
use crate::config::{Config, Role};
use crate::util::{self, git, git_ok, git_raw, GIT_LIMIT};
use anyhow::{bail, Result};
use std::io::Write;
use std::time::Duration;

pub fn run(role_name: &str, fresh: bool, create: bool) -> Result<i32> {
    let cfg = Config::load()?;
    let role = cfg.role(role_name)?;
    ensure_tree(&cfg, role, create)?;

    // One session per tree: a second writer in the same checkout is what the crew exists to prevent.
    if let Ok(sessions) = agent::list_sessions(&role.command) {
        if let Some(s) = agent::session_in(&sessions, &role.tree) {
            bail!(
                "a session is already running in {} ({}); use it, or stop it first",
                role.tree_text,
                s.name.as_deref().unwrap_or("unnamed")
            );
        }
    }

    sync_tree(role);
    print_charter(&cfg, role);
    print_board(&cfg);
    record_start(role).ok();

    let resume = !fresh && agent::has_conversation(&role.tree)?;
    if !fresh && !resume {
        println!(
            "(no conversation to continue in {}; starting a fresh one)",
            role.tree_text
        );
    }
    let _ = std::io::stdout().flush();
    let mut cmd = agent::start_command(role, resume);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        bail!("cannot run {}: {err}", role.command);
    }
    #[cfg(not(unix))]
    {
        // No exec on Windows: run the agent as a child and leave with its exit code.
        let status = cmd
            .status()
            .map_err(|e| anyhow::anyhow!("cannot run {}: {e}", role.command))?;
        Ok(status.code().unwrap_or(1))
    }
}

/// The tree exists, is a git checkout and is this role's; or it is created when configured and asked for.
pub fn ensure_tree(cfg: &Config, role: &Role, create: bool) -> Result<()> {
    if role.tree.is_dir() {
        if !git_ok(&role.tree, &["rev-parse", "--is-inside-work-tree"]) {
            bail!("{} is not a git checkout", role.tree_text);
        }
        if let Some(owner) = cfg.role_for(&role.tree) {
            if owner.name != role.name {
                bail!("{} is the tree of role {}", role.tree_text, owner.name);
            }
        }
        return Ok(());
    }
    let (Some(repo), Some(branch)) = (&role.worktree_of, &role.branch) else {
        bail!("the tree {} of role {} does not exist", role.tree_text, role.name);
    };
    if !repo.is_dir() || !git_ok(repo, &["rev-parse", "--is-inside-work-tree"]) {
        bail!(
            "the tree {} is missing and {} (worktree_of) is not a git checkout",
            role.tree_text,
            util::display_path(repo)
        );
    }
    let tree = role.tree.to_string_lossy().into_owned();
    let hint = format!(
        "git -C {} worktree add -b {branch} {tree} <start-point>",
        util::display_path(repo)
    );
    if !create {
        bail!(
            "the tree {} is missing; `crew start {} --create` runs: {hint}",
            role.tree_text,
            role.name
        );
    }
    let _ = git_raw(repo, &["fetch", "-q", "origin"], GIT_LIMIT);
    let start_point = default_remote_branch(repo).unwrap_or_else(|| "HEAD".into());
    if git_ok(repo, &["rev-parse", "--verify", "-q", &format!("refs/heads/{branch}")]) {
        git(repo, &["worktree", "add", &tree, branch])?;
    } else {
        git(repo, &["worktree", "add", "-b", branch, &tree, &start_point])?;
    }
    println!("created the worktree {} on {branch} from {start_point}", role.tree_text);
    Ok(())
}

/// `origin/<default>` when the repository knows its remote's default branch.
pub fn default_remote_branch(repo: &std::path::Path) -> Option<String> {
    git(repo, &["symbolic-ref", "-q", "--short", "refs/remotes/origin/HEAD"])
        .ok()
        .filter(|s| !s.is_empty())
}

/// Fetch when the tree is clean; fast-forward a plain checkout (a worktree keeps its branch where it is).
fn sync_tree(role: &Role) {
    let clean = git(&role.tree, &["status", "--porcelain"])
        .map(|s| s.is_empty())
        .unwrap_or(false);
    if !clean {
        return;
    }
    let has_remote = git(&role.tree, &["remote"]).map(|s| !s.is_empty()).unwrap_or(false);
    if !has_remote {
        return;
    }
    let fetched = git_raw(&role.tree, &["fetch", "-q"], Duration::from_secs(30))
        .map(|o| o.ok())
        .unwrap_or(false);
    if !fetched {
        eprintln!("(fetch failed in {}; continuing offline)", role.tree_text);
        return;
    }
    let plain = role.tree.join(".git").is_dir();
    if plain {
        let _ = git_raw(&role.tree, &["merge", "-q", "--ff-only", "@{u}"], GIT_LIMIT);
    }
}

fn print_charter(cfg: &Config, role: &Role) {
    let branch = git(&role.tree, &["branch", "--show-current"]).unwrap_or_default();
    let head = git(&role.tree, &["log", "--oneline", "-1"]).unwrap_or_default();
    println!(
        "== {} role of crew {} in {} ({} at {})",
        role.name,
        cfg.crew.name,
        role.tree_text,
        branch,
        util::first_words(&head, 8)
    );
    if !role.owns.is_empty() {
        println!("   owns: {}", role.owns);
    }
    if role.dispatcher {
        println!("   dispatcher: relays the user to the other roles; assigns nothing of its own, grants nothing");
    }
}

fn print_board(cfg: &Config) {
    let Ok(b) = Board::new(cfg) else { return };
    match b.refresh(Duration::from_secs(20)) {
        Ok(()) => {}
        Err(e) => eprintln!("(board: {e}; showing the last copy)"),
    }
    let content = b.read();
    let lines = board::tail(&content, 12);
    if lines.is_empty() {
        println!("-- board ({} on {}): no notes yet", b.path, b.repo);
        return;
    }
    println!("-- board ({} on {}), newest last:", b.path, b.repo);
    for l in lines {
        println!("{l}");
    }
}

/// Remember how the role was started, so `crew doctor` can compare it with the configuration.
pub fn record(role: &Role, mode: &str) -> Result<()> {
    let dir = util::cache_dir()?.join("started");
    std::fs::create_dir_all(&dir)?;
    let text = format!(
        "tree = {:?}\npermission_mode = {:?}\n",
        role.tree.to_string_lossy(),
        mode
    );
    std::fs::write(dir.join(&role.name), text)?;
    Ok(())
}

fn record_start(role: &Role) -> Result<()> {
    record(role, &role.permission_mode)
}

#[allow(dead_code)]
fn record_start_old(role: &Role) -> Result<()> {
    let dir = util::cache_dir()?.join("started");
    std::fs::create_dir_all(&dir)?;
    let text = format!(
        "tree = {:?}\npermission_mode = {:?}\n",
        role.tree.to_string_lossy(),
        role.permission_mode
    );
    std::fs::write(dir.join(&role.name), text)?;
    Ok(())
}

/// The permission mode `crew start` last used for the role, when it was recorded.
pub fn recorded_mode(role: &Role) -> Option<String> {
    let text = std::fs::read_to_string(util::cache_dir().ok()?.join("started").join(&role.name)).ok()?;
    let t: toml::Table = text.parse().ok()?;
    if t.get("tree")?.as_str()? != role.tree.to_string_lossy() {
        return None;
    }
    t.get("permission_mode")?.as_str().map(str::to_string)
}
