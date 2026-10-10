pub mod adopt;
pub mod answer;
pub mod ask;
pub mod board;
pub mod doctor;
pub mod handoff;
pub mod hook;
pub mod init;
pub mod msg;
pub mod note;
pub mod now;
pub mod open;
pub mod resource;
pub mod start;
pub mod status;
pub mod up;

use crate::config::{Config, Role};
use anyhow::Result;

/// Who signs a board line: the named role (which must exist), else the role owning the current
/// directory, else `user`.
pub fn signer(cfg: &Config, named: Option<&str>) -> Result<String> {
    if let Some(n) = named {
        return Ok(cfg.role(n)?.name.clone());
    }
    let here = std::env::current_dir()?;
    Ok(cfg
        .role_for(&here)
        .map(|r| r.name.clone())
        .unwrap_or_else(|| "user".into()))
}

/// The role owning the current directory, or the named one.
pub fn current_role<'a>(cfg: &'a Config, named: Option<&str>) -> Result<&'a Role> {
    if let Some(n) = named {
        return cfg.role(n);
    }
    let here = std::env::current_dir()?;
    cfg.role_for(&here).ok_or_else(|| {
        anyhow::anyhow!(
            "{} is in no role's tree; run inside one or pass --as <role> ({})",
            crate::util::display_path(&here),
            cfg.role_names().join(", ")
        )
    })
}
