use crate::board::{self, Board};
use crate::config::Config;
use crate::util::GIT_LIMIT;
use anyhow::Result;

pub fn run(
    n: usize,
    all: bool,
    sync_roles: bool,
    cached: bool,
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
