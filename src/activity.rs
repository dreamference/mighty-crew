//! What git says a role has done lately: commits on its branch since its last note, and how long
//! ago the last note and the last commit were. Productivity from the record, not from self-report.

use crate::util::git;
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use std::path::Path;

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Activity {
    /// Commits on the branch's upstream (or HEAD without one) dated at or after the last note.
    pub commits_since_note: Option<u64>,
    pub minutes_since_note: Option<i64>,
    pub minutes_since_commit: Option<i64>,
    /// The role's `crew now` line and its age, when it has one.
    pub now: Option<(String, i64)>,
}

/// A note's `YYYY-MM-DD HH:MM` as a local time.
pub fn note_time(when: &str) -> Option<DateTime<Local>> {
    let naive = NaiveDateTime::parse_from_str(when, "%Y-%m-%d %H:%M").ok()?;
    Local.from_local_datetime(&naive).single()
}

/// The ref that reflects what the role has pushed: the branch's upstream, else HEAD.
pub fn pushed_ref(tree: &Path) -> &'static str {
    if git(tree, &["rev-parse", "--verify", "-q", "@{u}"]).is_ok() {
        "@{u}"
    } else {
        "HEAD"
    }
}

pub fn for_role(role: &str, tree: &Path, last_note: Option<&str>, now: DateTime<Local>) -> Activity {
    let mut a = for_tree(tree, last_note, now);
    if let Some((when, text)) = crate::commands::now::read(role) {
        if let Some(t) = note_time(&when) {
            a.now = Some((text, (now - t).num_minutes()));
        }
    }
    a
}

pub fn for_tree(tree: &Path, last_note: Option<&str>, now: DateTime<Local>) -> Activity {
    let mut a = Activity::default();
    let note_at = last_note.and_then(note_time);
    a.minutes_since_note = note_at.map(|t| (now - t).num_minutes());
    if !tree.is_dir() {
        return a;
    }
    let r = pushed_ref(tree);
    if let Ok(ts) = git(tree, &["log", "-1", "--format=%ct", r]) {
        if let Ok(secs) = ts.trim().parse::<i64>() {
            a.minutes_since_commit = Some((now.timestamp() - secs) / 60);
        }
    }
    if let Some(t) = note_at {
        let since = format!("--since={}", t.timestamp());
        if let Ok(n) = git(tree, &["rev-list", "--count", &since, r]) {
            a.commits_since_note = n.trim().parse().ok();
        }
    }
    a
}

/// True when neither a note nor a commit is younger than `threshold` minutes.
pub fn is_quiet(a: &Activity, threshold: i64) -> bool {
    a.minutes_since_note.map(|m| m > threshold).unwrap_or(true)
        && a.minutes_since_commit.map(|m| m > threshold).unwrap_or(true)
        && a.now.as_ref().map(|(_, m)| *m > threshold).unwrap_or(true)
}

/// The `now` line for a table cell: the text and its age, or `-`.
pub fn describe_now(a: &Activity) -> String {
    match &a.now {
        Some((text, m)) => format!("{} ({})", crate::util::first_words(text, 6), ago(*m)),
        None => "-".into(),
    }
}

/// One short cell for `status`.
pub fn describe(a: &Activity) -> String {
    let mut parts = Vec::new();
    match (a.commits_since_note, a.minutes_since_note) {
        (Some(n), Some(m)) => parts.push(format!(
            "{n} commit{} since note {}",
            if n == 1 { "" } else { "s" },
            ago(m)
        )),
        (None, Some(m)) => parts.push(format!("note {}", ago(m))),
        _ => parts.push("no note".into()),
    }
    match a.minutes_since_commit {
        Some(m) => parts.push(format!("commit {}", ago(m))),
        None => parts.push("no commit".into()),
    }
    parts.join(", ")
}

fn ago(minutes: i64) -> String {
    if minutes < 0 {
        "just now".into()
    } else if minutes < 60 {
        format!("{minutes}m ago")
    } else if minutes < 60 * 48 {
        format!("{}h{:02} ago", minutes / 60, minutes % 60)
    } else {
        format!("{}d ago", minutes / (60 * 24))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_means_neither_is_young() {
        let a = Activity {
            commits_since_note: Some(0),
            minutes_since_note: Some(200),
            minutes_since_commit: Some(100),
            now: None,
        };
        assert!(is_quiet(&a, 90));
        assert!(!is_quiet(
            &Activity {
                minutes_since_commit: Some(10),
                ..a.clone()
            },
            90
        ));
        assert!(!is_quiet(
            &Activity {
                minutes_since_note: Some(10),
                ..a.clone()
            },
            90
        ));
        assert!(is_quiet(&Activity::default(), 90));
    }

    #[test]
    fn describes_in_a_few_words() {
        assert_eq!(
            describe(&Activity {
                commits_since_note: Some(2),
                minutes_since_note: Some(14),
                minutes_since_commit: Some(3),
                now: None,
            }),
            "2 commits since note 14m ago, commit 3m ago"
        );
        assert_eq!(describe(&Activity::default()), "no note, no commit");
        assert_eq!(
            describe(&Activity {
                minutes_since_note: Some(125),
                ..Default::default()
            }),
            "note 2h05 ago, no commit"
        );
        assert_eq!(ago(3000), "2d ago");
        assert_eq!(ago(-1), "just now");
    }

    #[test]
    fn note_times_parse_as_local() {
        let t = note_time("2026-10-10 16:02").unwrap();
        assert_eq!(t.format("%Y-%m-%d %H:%M").to_string(), "2026-10-10 16:02");
        assert!(note_time("yesterday").is_none());
    }
}
