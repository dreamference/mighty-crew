use crate::board::Board;
use crate::config::Config;
use anyhow::{bail, Result};

pub fn run(as_role: Option<&str>, text: &str) -> Result<i32> {
    let cfg = Config::load()?;
    let text = crate::util::one_line(text);
    if text.is_empty() {
        bail!("the note is empty");
    }
    let role = super::current_role(&cfg, as_role)?;
    let b = Board::new(&cfg)?;
    let line = b.append(&cfg, &role.name, &text)?;
    println!("{line}");
    Ok(0)
}
