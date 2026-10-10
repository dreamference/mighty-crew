use crate::config::Config;
use crate::util;
use anyhow::{bail, Result};
use std::process::Command;

const DEFAULT_TERMINALS: [&str; 4] = ["x-terminal-emulator", "gnome-terminal", "konsole", "xterm"];

/// The terminal command that will run `crew start <role>`, from the configuration or the machine.
pub fn find_terminal(cfg: &Config) -> Option<String> {
    let named: Vec<&str> = cfg.terminal.commands.iter().map(|s| s.as_str()).collect();
    let candidates = if named.is_empty() {
        DEFAULT_TERMINALS.to_vec()
    } else {
        named
    };
    candidates
        .into_iter()
        .find(|c| util::which(c).is_some())
        .map(str::to_string)
}

pub fn run(role: &str) -> Result<i32> {
    let cfg = Config::load()?;
    cfg.role(role)?;
    let exe = std::env::current_exe()?;
    let start = format!("{} start {role}", exe.display());
    let has_display = std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some();
    if !has_display {
        println!("no display; run in a terminal of your own:\n  {start}");
        return Ok(0);
    }
    let Some(term) = find_terminal(&cfg) else {
        bail!(
            "no terminal emulator found (tried {}); run `{start}` yourself",
            DEFAULT_TERMINALS.join(", ")
        );
    };
    let mut c = Command::new(&term);
    let base = std::path::Path::new(&term)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    match base.as_str() {
        "gnome-terminal" => {
            c.arg("--").arg(&exe).arg("start").arg(role);
        }
        _ => {
            c.arg("-e").arg(&exe).arg("start").arg(role);
        }
    }
    c.spawn().map_err(|e| anyhow::anyhow!("cannot run {term}: {e}"))?;
    println!("opened {role} in {term}");
    Ok(0)
}
