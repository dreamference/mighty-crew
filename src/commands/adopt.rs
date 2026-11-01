//! `crew adopt`: record the permission class of a session that was started by hand.

use crate::agent;
use crate::config::Config;
use anyhow::{bail, Result};

pub fn run(role_name: &str, mode: Option<&str>) -> Result<i32> {
    let cfg = Config::load()?;
    let role = cfg.role(role_name)?;
    let mode = mode.unwrap_or(&role.permission_mode);
    let running = agent::list_sessions(&role.command)
        .ok()
        .and_then(|l| agent::session_in_any(&l, &role.trees).map(|s| s.name.clone().unwrap_or_default()));
    let Some(name) = running else {
        bail!(
            "no session is running in {}; `crew start {}` records the class itself",
            role.tree_text,
            role.name
        );
    };
    super::start::record(role, mode)?;
    println!(
        "adopted: role {} runs session {name} in {mode}; doctor trusts this until the next `crew start {}`",
        role.name, role.name
    );
    if mode != role.permission_mode {
        eprintln!(
            "crew: {mode} is not the crew's class ({}); messages between the classes are held",
            cfg.agent.permission_mode
        );
    }
    Ok(0)
}
