//! `crew now`: what a role is doing right now, on this machine, outside the board.

use crate::config::Config;
use crate::util;
use anyhow::{bail, Result};
use std::path::PathBuf;

pub fn dir() -> Result<PathBuf> {
    Ok(util::cache_dir()?.join("now"))
}

/// The role's line: (when, text), if any.
pub fn read(role: &str) -> Option<(String, String)> {
    let text = std::fs::read_to_string(dir().ok()?.join(role)).ok()?;
    let (when, doing) = text.split_once('\n')?;
    Some((when.trim().to_string(), doing.trim().to_string()))
}

pub fn run(text: Option<&str>, as_role: Option<&str>) -> Result<i32> {
    let cfg = Config::load()?;
    match text {
        None => {
            for role in &cfg.roles {
                match read(&role.name) {
                    Some((when, doing)) => println!("{:<12} {when}  {doing}", role.name),
                    None => println!("{:<12} -", role.name),
                }
            }
            Ok(0)
        }
        Some(t) => {
            let t = util::one_line(t);
            if t.is_empty() {
                bail!("say what you are doing, in one line");
            }
            let role = super::current_role(&cfg, as_role)?;
            let d = dir()?;
            std::fs::create_dir_all(&d)?;
            let when = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
            std::fs::write(d.join(&role.name), format!("{when}\n{t}\n"))?;
            println!("{} now: {t}", role.name);
            Ok(0)
        }
    }
}
