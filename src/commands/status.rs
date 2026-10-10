use crate::agent;
use crate::board::{self, Board};
use crate::config::Config;
use crate::util::{self, git, GIT_LIMIT};
use anyhow::Result;
use std::collections::HashMap;

pub fn run() -> Result<i32> {
    let cfg = Config::load()?;
    let b = Board::new(&cfg)?;
    let board_text = match b.refresh(GIT_LIMIT) {
        Ok(()) => b.read(),
        Err(e) => {
            eprintln!("(board: {e}; using the last copy)");
            b.read()
        }
    };
    let mut listings: HashMap<String, Vec<agent::Session>> = HashMap::new();
    let mut rows: Vec<[String; 6]> = vec![[
        "role".into(),
        "tree".into(),
        "branch".into(),
        "head".into(),
        "session".into(),
        "last note".into(),
    ]];
    for role in &cfg.roles {
        let (branch, head) = if role.tree.is_dir() {
            (
                git(&role.tree, &["branch", "--show-current"])
                    .map(|s| if s.is_empty() { "(detached)".into() } else { s })
                    .unwrap_or_else(|_| "?".into()),
                git(&role.tree, &["log", "--oneline", "-1"])
                    .map(|s| util::first_words(&s, 5))
                    .unwrap_or_else(|_| "?".into()),
            )
        } else {
            ("(missing)".into(), "".into())
        };
        let sessions = listings.entry(role.command.clone()).or_insert_with(|| {
            agent::list_sessions(&role.command).unwrap_or_else(|e| {
                eprintln!("({e})");
                Vec::new()
            })
        });
        let session = match agent::session_in(sessions, &role.tree) {
            Some(s) => format!("{} ({})", s.describe(), s.name.as_deref().unwrap_or("unnamed")),
            None => "not running".into(),
        };
        let note = board::last_note(&board_text, &role.name)
            .and_then(|l| board::parse_note(l).map(|(when, _, text)| format!("{when} {}", util::first_words(text, 8))))
            .unwrap_or_else(|| "-".into());
        rows.push([role.name.clone(), role.tree_text.clone(), branch, head, session, note]);
    }
    let mut widths = [0usize; 6];
    for r in &rows {
        for (i, c) in r.iter().enumerate() {
            widths[i] = widths[i].max(c.chars().count());
        }
    }
    for r in &rows {
        let mut line = String::new();
        for (i, c) in r.iter().enumerate() {
            if i == 5 {
                line.push_str(c);
            } else {
                line.push_str(&format!("{:w$}  ", c, w = widths[i]));
            }
        }
        println!("{}", line.trim_end());
    }
    Ok(0)
}
