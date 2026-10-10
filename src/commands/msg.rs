use crate::agent::{self, Delivery};
use crate::board::Board;
use crate::config::Config;
use anyhow::{bail, Result};

const MAX_CHARS: usize = 300;

pub fn run(role: &str, text: &str) -> Result<i32> {
    let cfg = Config::load()?;
    let target = cfg.role(role)?;
    let text = crate::util::one_line(text);
    if text.is_empty() {
        bail!("the message is empty");
    }
    if text.chars().count() > MAX_CHARS {
        bail!(
            "the message is {} characters; keep it under {MAX_CHARS} and put the rest on the board or in a file",
            text.chars().count()
        );
    }
    let sender = std::env::current_dir()
        .ok()
        .and_then(|d| cfg.role_for(&d).map(|r| r.name.clone()));
    if sender.as_deref() == Some(role) {
        bail!("{role} is the role you are in; message another role");
    }
    // The durable twin: the same text on the board, so a held or expired message is not lost.
    let twin = format!("to {role}: {text}");
    // Written first and fail-closed: a message the board does not hold is not sent.
    let line = Board::new(&cfg)
        .and_then(|b| b.append(&cfg, sender.as_deref().unwrap_or("user"), &twin))
        .map_err(|e| anyhow::anyhow!("the board did not take the line, so nothing was sent: {e:#}"))?;
    println!("{line}");
    let sessions = agent::list_sessions(&target.command)?;
    let Some(session) = agent::session_in(&sessions, &target.tree) else {
        bail!(
            "no session is running in {} ({}); start one with `crew start {}`",
            target.tree_text,
            role,
            role
        );
    };
    let name = session.name.clone().unwrap_or_else(|| role.to_string());
    let sender_mode = sender
        .as_deref()
        .and_then(|s| cfg.role(s).ok())
        .map(|r| r.permission_mode.clone())
        .unwrap_or_else(|| cfg.agent.permission_mode.clone());
    let (delivery, report) = agent::send(&target.command, &sender_mode, &cfg.agent.msg_model, &name, &text)?;
    match delivery {
        Delivery::Delivered => {
            println!("in the inbox of {role} ({name}); its agent reads it in its next turn: {report}");
            Ok(0)
        }
        Delivery::Held => {
            println!("held by {role} ({name}) for its user's approval: {report}");
            eprintln!("crew: the message was held; the roles are in different permission classes. Not retried.");
            Ok(1)
        }
        Delivery::Refused => {
            eprintln!("crew: {role} ({name}) refused the message: {report}");
            Ok(1)
        }
    }
}
