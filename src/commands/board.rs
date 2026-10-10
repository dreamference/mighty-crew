use crate::board::{self, Board};
use crate::config::Config;
use crate::util::GIT_LIMIT;
use anyhow::Result;

pub fn run(n: usize, all: bool, sync_roles: bool, cached: bool, for_role: Option<&str>) -> Result<i32> {
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
    let lines = match for_role {
        Some(r) => {
            cfg.role(r)?;
            let mine = board::lines_for(&content, r);
            if all {
                mine
            } else {
                mine.into_iter()
                    .rev()
                    .take(n)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect()
            }
        }
        None if all => board::status_lines(&content),
        None => board::tail(&content, n),
    };
    if lines.is_empty() {
        println!("(no notes yet)");
    }
    for l in lines {
        println!("{l}");
    }
    Ok(0)
}
