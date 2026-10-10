pub mod board;
pub mod doctor;
pub mod hook;
pub mod init;
pub mod msg;
pub mod note;
pub mod open;
pub mod start;
pub mod status;

use crate::config::{Config, Role};
use anyhow::Result;

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
