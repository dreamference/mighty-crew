//! `crew up`: from a clone to a live crew in one command.

use crate::agent;
use crate::config::{Config, Role};
use crate::util;
use anyhow::{bail, Result};
use std::os::unix::process::CommandExt;
use std::process::Command;

pub fn run(fresh: bool) -> Result<i32> {
    let cfg = Config::load()?;
    let exe = std::env::current_exe()?;
    let mut failed = 0;
    let mut to_open: Vec<&Role> = Vec::new();
    for role in &cfg.roles {
        match super::start::ensure_tree(&cfg, role, true) {
            Ok(()) => {}
            Err(e) => {
                eprintln!("crew: {e:#}");
                failed += 1;
                continue;
            }
        }
        let live = agent::list_sessions(&role.command)
            .ok()
            .and_then(|l| agent::session_in(&l, &role.tree).map(|s| s.name.clone().unwrap_or_default()));
        match live {
            Some(name) => println!("{}: already running ({name})", role.name),
            None => to_open.push(role),
        }
    }
    let start_args = |r: &Role| {
        let mut v = vec![exe.to_string_lossy().into_owned(), "start".into(), r.name.clone()];
        if fresh {
            v.push("--new".into());
        }
        v
    };
    if to_open.is_empty() {
        println!("every role is running");
    } else if let Some(tmux) = util::which("tmux") {
        let session = format!("crew.{}", cfg.crew.name);
        let has = Command::new(&tmux)
            .args(["has-session", "-t", &session])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !has {
            for (i, r) in to_open.iter().enumerate() {
                let cmd = start_args(r)
                    .iter()
                    .map(|a| shell_quote(a))
                    .collect::<Vec<_>>()
                    .join(" ");
                let mut c = Command::new(&tmux);
                if i == 0 {
                    c.args(["new-session", "-d", "-s", &session, "-n", &r.name, &cmd]);
                } else {
                    c.args(["new-window", "-t", &session, "-n", &r.name, &cmd]);
                }
                let out = c.output()?;
                if !out.status.success() {
                    bail!("tmux: {}", util::one_line(&String::from_utf8_lossy(&out.stderr)));
                }
                println!("{}: started in tmux window {}:{}", r.name, session, r.name);
            }
        } else {
            println!(
                "tmux session {session} exists; attaching (roles not running there: {})",
                to_open.iter().map(|r| r.name.as_str()).collect::<Vec<_>>().join(", ")
            );
        }
        if failed > 0 {
            eprintln!("crew: {failed} role(s) could not be prepared");
        }
        let mut c = Command::new(&tmux);
        if std::env::var_os("TMUX").is_some() {
            c.args(["switch-client", "-t", &session]);
        } else {
            c.args(["attach", "-t", &session]);
        }
        let err = c.exec();
        bail!("cannot run tmux: {err}");
    } else if std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some() {
        for r in &to_open {
            match super::open::run(&r.name) {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("crew: {e:#}");
                    failed += 1;
                }
            }
        }
    } else {
        println!("no tmux and no display; run one per terminal:");
        for r in &to_open {
            println!("  {}", start_args(r).join(" "));
        }
    }
    Ok(if failed > 0 { 1 } else { 0 })
}

fn shell_quote(s: &str) -> String {
    if s.chars().all(|c| c.is_ascii_alphanumeric() || "/._-+=:,@".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}
