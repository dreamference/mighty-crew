use crate::board::{self, Board};
use crate::config::Config;
use crate::util::GIT_LIMIT;
use anyhow::Result;

pub fn run(
    n: usize,
    all: bool,
    sync_roles: bool,
    cached: bool,
    pending: bool,
    for_role: Option<&str>,
    tag: Option<&str>,
) -> Result<i32> {
    let cfg = Config::load()?;
    let b = Board::new(&cfg)?;
    if sync_roles {
        if b.sync_roles(&cfg)? {
            println!(
                "board: role table rewritten from {}",
                crate::util::display_path(&cfg.path)
            );
        } else {
            println!("board: role table already current");
        }
        return Ok(0);
    }
    if cached {
        b.ensure_clone(GIT_LIMIT)?;
    } else {
        b.refresh(GIT_LIMIT)?;
    }
    if pending {
        let doc = b.doc(&cfg);
        if doc.pending.is_empty() {
            println!("(no decisions pending)");
            return Ok(0);
        }
        for d in &doc.pending {
            println!("{} · {} · asked by {}: {}", d.id, d.when, d.role, d.question);
            for o in &d.options {
                println!(
                    "  - {o}{}",
                    if d.recommended.as_deref() == Some(o.as_str()) {
                        " (recommended)"
                    } else {
                        ""
                    }
                );
            }
            if let Some(t) = &d.ticked {
                println!("  answered on the forge: {t} (recorded as a note at the next board write)");
            }
        }
        println!("answer with `crew answer <id> <option>`{}", super::ask::link_suffix(&b));
        return Ok(0);
    }
    let content = b.read();
    if !b.exists() {
        println!(
            "(the board {} does not exist yet on {}; the first `crew note` creates it)",
            b.path, b.repo
        );
        return Ok(0);
    }
    let tag = tag.map(|t| t.trim_end_matches(':'));
    let mut lines = match (for_role, tag) {
        (Some(r), _) => {
            cfg.role(r)?;
            let mut mine = board::lines_for(&content, r);
            if let Some(t) = tag {
                mine.retain(|l| board::parse_note(l).and_then(|(_, _, text)| board::tag_of(text)) == Some(t));
            }
            mine
        }
        (None, Some(t)) => board::lines_tagged(&content, t),
        (None, None) => board::status_lines(&content),
    };
    if !all {
        let skip = lines.len().saturating_sub(n);
        lines = lines.into_iter().skip(skip).collect();
    }
    if lines.is_empty() {
        println!(
            "{}",
            if tag.is_some() || for_role.is_some() {
                "(no matching notes)"
            } else {
                "(no notes yet)"
            }
        );
    }
    for l in lines {
        println!("{l}");
    }
    Ok(0)
}
