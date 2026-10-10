//! `crew handoff`: data for another role goes through a neutral folder, never its tree.

use crate::board::Board;
use crate::config::Config;
use crate::util;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

pub fn dir() -> Result<PathBuf> {
    Ok(util::cache_dir()?.join("handoff"))
}

fn copy_tree(from: &Path, to: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    if from.is_dir() {
        std::fs::create_dir_all(to)?;
        for e in std::fs::read_dir(from)? {
            let e = e?;
            copy_tree(&e.path(), &to.join(e.file_name()), files)?;
        }
    } else {
        if let Some(p) = to.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::copy(from, to).with_context(|| format!("cannot copy {}", from.display()))?;
        files.push(to.to_path_buf());
    }
    Ok(())
}

pub fn run(role_name: &str, path: &str, as_role: Option<&str>) -> Result<i32> {
    let cfg = Config::load()?;
    let to = cfg.role(role_name)?;
    let from = super::signer(&cfg, as_role)?;
    let src = util::normalize(&util::expand_tilde(path)?);
    if !src.exists() {
        bail!("{} does not exist", util::display_path(&src));
    }
    let name = src
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "handoff".into());
    let dest = dir()?.join(&from).join(&to.name).join(&name);
    if dest.exists() {
        bail!(
            "{} exists already; remove it or hand off under another name",
            util::display_path(&dest)
        );
    }
    let mut files = Vec::new();
    copy_tree(&src, &dest, &mut files)?;
    let scrub = files
        .iter()
        .filter(|f| std::fs::read_to_string(f).map(|t| t.contains("/tmp/")).unwrap_or(false))
        .count();
    let b = Board::new(&cfg)?;
    let line = b.append(
        &cfg,
        &from,
        &format!(
            "to {}: handoff {name} at {} ({} files)",
            to.name,
            util::display_path(&dest),
            files.len()
        ),
    )?;
    println!("{line}");
    println!("copied to {} for {}", util::display_path(&dest), to.name);
    if scrub > 0 {
        println!("scrub before anything reaches a public repository: {scrub} file(s) mention /tmp/ paths");
    } else {
        println!("scrub temporary paths and names before anything reaches a public repository");
    }
    Ok(0)
}
