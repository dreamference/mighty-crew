//! `crew resource`: a shared resource and the rules for using it, on the chat.

use crate::chat::Chat;
use crate::config::Config;
use crate::util;
use anyhow::{bail, Result};

pub fn run(name: Option<&str>, rules: &str, as_role: Option<&str>) -> Result<i32> {
    let cfg = Config::load()?;
    let b = Chat::new(&cfg)?;
    let Some(name) = name else {
        b.refresh(util::GIT_LIMIT)?;
        let doc = b.doc(&cfg);
        if doc.resources.is_empty() {
            println!("(no resources declared; `crew resource <name> \"<rules>\"` adds one)");
        }
        for r in &doc.resources {
            println!("{} ({}): {}", r.name, r.owner, r.rules);
        }
        return Ok(0);
    };
    let name = util::one_line(name);
    let rules = util::one_line(rules);
    if name.is_empty() || rules.is_empty() {
        bail!("a resource needs a name and one line of rules");
    }
    let owner = super::current_role(&cfg, as_role)?.name.clone();
    let n = name.clone();
    let o = owner.clone();
    let r = rules.clone();
    let written = b.write(&cfg, &format!("chat: resource {name}"), move |doc| {
        Ok(doc.upsert_resource(&n, &o, &r))
    })?;
    match written {
        Some(ticked) => {
            for t in ticked {
                println!("recorded from the forge: {t}");
            }
            println!("{name} ({owner}): {rules}");
        }
        None => println!("{name} already says that"),
    }
    Ok(0)
}
