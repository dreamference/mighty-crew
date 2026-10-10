use crate::activity;
use crate::agent;
use crate::chat::{self, Chat};
use crate::config::Config;
use crate::util::{self, git, GIT_LIMIT};
use anyhow::Result;
use std::collections::HashMap;

pub fn run() -> Result<i32> {
    let cfg = Config::load()?;
    let b = Chat::new(&cfg)?;
    let chat_text = match b.refresh(GIT_LIMIT) {
        Ok(()) => b.read(),
        Err(e) => {
            eprintln!("(chat: {e}; using the last copy)");
            b.read()
        }
    };
    let mut listings: HashMap<String, Vec<agent::Session>> = HashMap::new();
    let now = chrono::Local::now();
    let mut rows: Vec<[String; 8]> = vec![[
        "role".into(),
        "tree".into(),
        "branch".into(),
        "head".into(),
        "session".into(),
        "activity".into(),
        "now".into(),
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
        let last = chat::last_note(&chat_text, &role.name).and_then(chat::parse_note);
        let note = last
            .map(|(when, _, text)| format!("{when} {}", util::first_words(text, 8)))
            .unwrap_or_else(|| "-".into());
        let a = activity::for_role(&role.name, &role.tree, last.map(|(when, _, _)| when), now);
        let activity = activity::describe(&a);
        let now_cell = activity::describe_now(&a);
        rows.push([
            role.name.clone(),
            role.tree_text.clone(),
            branch,
            head,
            session,
            activity,
            now_cell,
            note,
        ]);
    }
    let mut widths = [0usize; 8];
    for r in &rows {
        for (i, c) in r.iter().enumerate() {
            widths[i] = widths[i].max(c.chars().count());
        }
    }
    for r in &rows {
        let mut line = String::new();
        for (i, c) in r.iter().enumerate() {
            if i == 7 {
                line.push_str(c);
            } else {
                line.push_str(&format!("{:w$}  ", c, w = widths[i]));
            }
        }
        println!("{}", line.trim_end());
    }
    let doc = b.doc(&cfg);
    if !doc.resources.is_empty() {
        println!(
            "resources: {}",
            doc.resources
                .iter()
                .map(|r| format!("{} ({}): {}", r.name, r.owner, util::first_words(&r.rules, 8)))
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
    if !doc.pending.is_empty() {
        println!(
            "{} decision(s) pending: `crew chat --pending`{}",
            doc.pending.len(),
            super::ask::link_suffix(&b)
        );
    }
    Ok(0)
}
