pub mod adopt;
pub mod answer;
pub mod ask;
pub mod chat;
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

/// Who signs a chat line: the named role (which must exist), else the role owning the current
/// directory, else `user`.
pub fn signer(cfg: &Config, named: Option<&str>) -> Result<String> {
    if let Some(n) = named {
        return Ok(cfg.role(n)?.name.clone());
    }
    let here = std::env::current_dir()?;
    match cfg.lookup(&here) {
        Err(why) => Err(anyhow::anyhow!("{why}")),
        Ok(r) => Ok(r.map(|r| r.name.clone()).unwrap_or_else(|| "user".into())),
    }
}

/// The role owning the current directory, or the named one.
pub fn current_role<'a>(cfg: &'a Config, named: Option<&str>) -> Result<&'a Role> {
    if let Some(n) = named {
        return cfg.role(n);
    }
    let here = std::env::current_dir()?;
    match cfg.lookup(&here) {
        Err(why) => Err(anyhow::anyhow!("{why}")),
        Ok(Some(r)) => Ok(r),
        Ok(None) => Err(anyhow::anyhow!(
            "{} is in no role's tree; run inside one or pass --as <role> ({})",
            crate::util::display_path(&here),
            cfg.role_names().join(", ")
        )),
    }
}
