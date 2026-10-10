//! `crew answer`: record the user's choice on a pending decision as a `decision:` note.

use crate::board::Board;
use crate::config::Config;
use crate::sections;
use anyhow::{bail, Result};

pub fn run(id: &str, option: &str, as_role: Option<&str>) -> Result<i32> {
    let cfg = Config::load()?;
    let by = super::signer(&cfg, as_role)?;
    let b = Board::new(&cfg)?;
    b.refresh(crate::util::GIT_LIMIT)?;
    let doc = b.doc(&cfg);
    let Some(d) = doc.pending.iter().find(|d| d.id == id) else {
        let open: Vec<&str> = doc.pending.iter().map(|d| d.id.as_str()).collect();
        bail!(
            "no pending decision {id}; open: {}",
            if open.is_empty() {
                "none".into()
            } else {
                open.join(", ")
            }
        );
    };
    let Some(chosen) = d.options.iter().find(|o| o.eq_ignore_ascii_case(option)) else {
        bail!(
            "{id} has no option {option:?}; the options are: {}",
            d.options.join(", ")
        );
    };
    let chosen = chosen.clone();
    let mut note = String::new();
    let ticked = b.write(&cfg, &format!("board: answer {id}"), |doc| {
        let Some(pos) = doc.pending.iter().position(|d| d.id == id) else {
            return Ok(false);
        };
        let d = doc.pending.remove(pos);
        note = sections::decision_note(&by, &d, &chosen);
        doc.notes.push(note.clone());
        Ok(true)
    })?;
    match ticked {
        None => bail!("{id} was answered on the forge in the meantime; see `crew board --decisions`"),
        Some(others) => {
            for t in others {
                println!("recorded from the forge: {t}");
            }
        }
    }
    println!("{note}");
    Ok(0)
}
