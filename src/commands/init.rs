use crate::config::{self, Identity};
use crate::util::{self, git};
use anyhow::{bail, Result};

pub fn run(force: bool) -> Result<i32> {
    let path = util::config_path()?;
    if path.exists() && !force {
        bail!(
            "{} exists; edit it, or pass --force to overwrite it",
            util::display_path(&path)
        );
    }
    let here = std::env::current_dir()?;
    let Ok(top) = git(&here, &["rev-parse", "--show-toplevel"]) else {
        bail!(
            "{} is not inside a git repository; run `crew init` in the repository that will hold the chat",
            util::display_path(&here)
        );
    };
    let top = util::normalize(std::path::Path::new(&top));
    let Ok(url) = git(&top, &["remote", "get-url", "origin"]) else {
        bail!("the repository has no `origin` remote; the chat needs one every role can push to");
    };
    let branch = super::start::default_remote_branch(&top)
        .map(|b| b.trim_start_matches("origin/").to_string())
        .or_else(|| git(&top, &["branch", "--show-current"]).ok().filter(|s| !s.is_empty()))
        .unwrap_or_else(|| "main".into());
    let identity = match (
        git(&top, &["config", "user.name"]),
        git(&top, &["config", "user.email"]),
    ) {
        (Ok(name), Ok(email)) if !name.is_empty() && !email.is_empty() => Some(Identity { name, email }),
        _ => None,
    };
    let base = top
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "crew".into());
    let crew_name = base.clone();
    let role_name: String = base
        .to_ascii_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let text = config::render_initial(
        &crew_name,
        &url,
        &branch,
        identity.as_ref(),
        &role_name,
        &util::display_path(&top),
    );
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, &text)?;
    config::Config::parse(&text, &path)?;
    println!("wrote {}", util::display_path(&path));
    println!("next: add a [roles.<name>] table per role (each with its own tree), then `crew install-hook` and `crew doctor`");
    Ok(0)
}
