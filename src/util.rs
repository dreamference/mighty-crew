//! Paths, environment and the git runner shared by every command.

use anyhow::{anyhow, bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The user's home directory, from `HOME`.
pub fn home() -> Result<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| {
            if cfg!(windows) {
                std::env::var_os("USERPROFILE")
            } else {
                None
            }
        })
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| anyhow!("HOME is not set"))
}

/// `~/.config/mighty-crew/crew.toml`, or `$XDG_CONFIG_HOME/mighty-crew/crew.toml`,
/// or whatever `MIGHTY_CREW_CONFIG` names.
pub fn config_path() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("MIGHTY_CREW_CONFIG").filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    let base = match std::env::var_os("XDG_CONFIG_HOME").filter(|p| !p.is_empty()) {
        Some(p) => PathBuf::from(p),
        None => home()?.join(".config"),
    };
    Ok(base.join("mighty-crew").join("crew.toml"))
}

/// `~/.cache/mighty-crew` or `$XDG_CACHE_HOME/mighty-crew`.
pub fn cache_dir() -> Result<PathBuf> {
    let base = match std::env::var_os("XDG_CACHE_HOME").filter(|p| !p.is_empty()) {
        Some(p) => PathBuf::from(p),
        None => home()?.join(".cache"),
    };
    Ok(base.join("mighty-crew"))
}

/// Claude Code's configuration directory: `CLAUDE_CONFIG_DIR` or `~/.claude`.
pub fn claude_config_dir() -> Result<PathBuf> {
    if let Some(p) = std::env::var_os("CLAUDE_CONFIG_DIR").filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    Ok(home()?.join(".claude"))
}

/// Expand a leading `~` or `~/` to the home directory.
pub fn expand_tilde(s: &str) -> Result<PathBuf> {
    if s == "~" {
        return home();
    }
    if let Some(rest) = s.strip_prefix("~/") {
        return Ok(home()?.join(rest));
    }
    Ok(PathBuf::from(s))
}

/// Absolute, with `.` and `..` removed lexically; canonical when the path exists.
pub fn normalize(p: &Path) -> PathBuf {
    if let Ok(c) = p.canonicalize() {
        return c;
    }
    let abs = if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|d| d.join(p))
            .unwrap_or_else(|_| p.to_path_buf())
    };
    let mut out = PathBuf::new();
    for c in abs.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// A path for printing: the home directory shown as `~`.
pub fn display_path(p: &Path) -> String {
    if let Ok(h) = home() {
        if let Ok(rest) = p.strip_prefix(&h) {
            if rest.as_os_str().is_empty() {
                return "~".to_string();
            }
            return format!("~/{}", rest.display());
        }
    }
    p.display().to_string()
}

/// The first directory on `PATH` holding an executable of that name, or the path itself.
pub fn which(cmd: &str) -> Option<PathBuf> {
    #[cfg(unix)]
    fn executable(p: &Path) -> bool {
        use std::os::unix::fs::PermissionsExt;
        p.is_file()
            && p.metadata()
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
    }
    #[cfg(not(unix))]
    fn executable(p: &Path) -> bool {
        p.is_file()
    }
    // On Windows a command name resolves through PATHEXT (`claude.cmd`, `crew.exe`).
    let suffixes: Vec<String> = if cfg!(windows) {
        let exts = std::env::var("PATHEXT").unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".into());
        std::iter::once(String::new())
            .chain(exts.split(';').map(|e| e.to_string()))
            .collect()
    } else {
        vec![String::new()]
    };
    if cmd.contains('/') || cmd.contains('\\') {
        let p = expand_tilde(cmd).ok()?;
        return suffixes
            .iter()
            .map(|x| PathBuf::from(format!("{}{x}", p.display())))
            .find(|p| executable(p));
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .flat_map(|d| suffixes.iter().map(move |x| d.join(format!("{cmd}{x}"))))
        .find(|p| executable(p))
}

/// Output of a finished process.
pub struct Output {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl Output {
    pub fn ok(&self) -> bool {
        self.status == Some(0)
    }
}

/// Run a command, capturing both streams, with a wall-clock limit counted from the spawn.
/// A process still running at the limit is killed and reported with `status: None`.
pub fn run(cmd: &mut Command, limit: Duration) -> Result<Output> {
    let program = cmd.get_program().to_string_lossy().into_owned();
    let start = Instant::now();
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("cannot run {program}"))?;
    let out = child.stdout.take().unwrap();
    let err = child.stderr.take().unwrap();
    let out_reader = std::thread::spawn(move || read_all(out));
    let err_reader = std::thread::spawn(move || read_all(err));
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break Some(s);
        }
        if start.elapsed() > limit {
            let _ = child.kill();
            let _ = child.wait();
            break None;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let stdout = out_reader.join().unwrap_or_default();
    let stderr = err_reader.join().unwrap_or_default();
    Ok(Output {
        status: status.and_then(|s| s.code()),
        stdout,
        stderr,
    })
}

fn read_all(mut pipe: impl std::io::Read) -> String {
    let mut s = String::new();
    let _ = pipe.read_to_string(&mut s);
    s
}

pub const GIT_LIMIT: Duration = Duration::from_secs(60);

/// Run `git -C dir args…` and return its stdout, trimmed; the error carries stderr.
pub fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = git_raw(dir, args, GIT_LIMIT)?;
    if !out.ok() {
        let why = out.stderr.trim();
        let why = if why.is_empty() { "failed" } else { why };
        bail!("git {}: {why}", args.join(" "));
    }
    Ok(out.stdout.trim_end().to_string())
}

pub fn git_raw(dir: &Path, args: &[&str], limit: Duration) -> Result<Output> {
    git_raw_env(dir, args, &[], limit)
}

/// `git_raw` with environment variables set for the one command (an author identity, say).
pub fn git_raw_env(dir: &Path, args: &[&str], env: &[(&str, &str)], limit: Duration) -> Result<Output> {
    let mut c = Command::new("git");
    c.arg("-C").arg(dir).args(args);
    for (k, v) in env {
        c.env(k, v);
    }
    run(&mut c, limit)
}

/// `git` succeeded, with no interest in the output.
pub fn git_ok(dir: &Path, args: &[&str]) -> bool {
    git_raw(dir, args, GIT_LIMIT).map(|o| o.ok()).unwrap_or(false)
}

/// One line: whitespace runs folded to one space, ends trimmed.
pub fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The first `n` words of a string.
pub fn first_words(s: &str, n: usize) -> String {
    let words: Vec<&str> = s.split_whitespace().collect();
    let mut out = words.iter().take(n).cloned().collect::<Vec<_>>().join(" ");
    if words.len() > n {
        out.push('…');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_line_folds_whitespace() {
        assert_eq!(one_line("  a \n\n b\t c  "), "a b c");
        assert_eq!(one_line(""), "");
    }

    #[test]
    fn first_words_marks_the_cut() {
        assert_eq!(first_words("one two three", 2), "one two…");
        assert_eq!(first_words("one two", 2), "one two");
        assert_eq!(first_words("", 3), "");
    }

    #[test]
    fn normalize_resolves_dots_lexically_for_missing_paths() {
        assert_eq!(normalize(Path::new("/a/b/../c/./d")), PathBuf::from("/a/c/d"));
        assert_eq!(normalize(Path::new("/")), PathBuf::from("/"));
    }

    #[test]
    fn run_kills_a_process_past_its_limit() {
        let start = Instant::now();
        let out = run(Command::new("sleep").arg("5"), Duration::from_millis(300)).unwrap();
        assert_eq!(out.status, None);
        assert!(start.elapsed() < Duration::from_secs(3), "took {:?}", start.elapsed());
    }

    #[test]
    fn run_captures_both_streams_and_the_status() {
        let out = run(
            Command::new("sh").args(["-c", "echo out; echo err >&2; exit 3"]),
            Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(out.status, Some(3));
        assert_eq!(out.stdout, "out\n");
        assert_eq!(out.stderr, "err\n");
        assert!(!out.ok());
        assert!(run(&mut Command::new("/nonexistent/program"), Duration::from_secs(1)).is_err());
    }

    #[test]
    #[cfg(unix)]
    fn which_finds_executables_on_path_and_by_path() {
        let dir = std::env::temp_dir().join(format!("crew-which-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("thing");
        std::fs::write(&exe, "#!/bin/sh\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(dir.join("plain"), "").unwrap();
        let saved = std::env::var_os("PATH");
        std::env::set_var("PATH", &dir);
        assert_eq!(which("thing"), Some(exe.clone()));
        assert_eq!(which("plain"), None);
        assert_eq!(which("missing"), None);
        assert_eq!(which(exe.to_str().unwrap()), Some(exe.clone()));
        if let Some(p) = saved {
            std::env::set_var("PATH", p);
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
