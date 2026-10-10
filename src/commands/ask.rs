//! `crew ask`: post a decision for the user on the board, under `## Decisions pending`.

use crate::board::Board;
use crate::config::Config;
use crate::sections::{self, Decision};
use crate::util;
use anyhow::{bail, Result};

pub fn run(question: &str, options: &[String], recommend: Option<&str>, as_role: Option<&str>) -> Result<i32> {
    let cfg = Config::load()?;
    let question = util::one_line(question);
    if question.is_empty() {
        bail!("the question is empty");
    }
    let options: Vec<String> = options
        .iter()
        .flat_map(|o| o.split(',').map(util::one_line))
        .filter(|o| !o.is_empty())
        .collect();
    if options.len() < 2 {
        bail!("give at least two options: --options a,b,c");
    }
    if let Some(r) = recommend {
        if !options.iter().any(|o| o == r) {
            bail!("the recommended option {r:?} is not among the options");
        }
    }
    let role = super::signer(&cfg, as_role)?;
    let b = Board::new(&cfg)?;
    let mut id = String::new();
    let q = question.clone();
    let opts = options.clone();
    let rec = recommend.map(str::to_string);
    let r = role.clone();
    let ticked = b.write(&cfg, &format!("board: ask {role}"), |doc| {
        id = doc.next_id();
        doc.pending.push(Decision {
            id: id.clone(),
            when: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(),
            role: r.clone(),
            question: q.clone(),
            options: opts.clone(),
            recommended: rec.clone(),
            ticked: None,
        });
        Ok(true)
    })?;
    for t in ticked.unwrap_or_default() {
        println!("recorded from the forge: {t}");
    }
    println!("{id} asked by {role}: {question}");
    for o in &options {
        println!(
            "  - {o}{}",
            if recommend == Some(o.as_str()) {
                " (recommended)"
            } else {
                ""
            }
        );
    }
    println!(
        "answer with `crew answer {id} <option>`, or tick the box on the forge{}",
        link_suffix(&b)
    );
    Ok(0)
}

pub fn link_suffix(b: &Board) -> String {
    match sections::forge_link(&b.repo, &b.branch, &b.path) {
        Some(l) => format!(": {l}"),
        None => String::new(),
    }
}
