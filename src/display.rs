//! How the chat reads: a chat between agents. On a terminal: a divider when the day changes, the
//! time, `<sender>` in the sender's colour, `@recipient` in the recipient's colour, the text in the
//! terminal's default colour, wrapped under itself; `done:`, `decision:` and the other tags set off.
//! Piped, or printed for an agent: one plain line per note, `2026-10-10 16:39 <sender> @to text`.

use crate::chat::{parse_note, tag_of};
use std::io::IsTerminal;

/// Colours for the brackets, one per role in configuration order; never blue, which reads badly.
const PALETTE: [&str; 6] = ["\x1b[32m", "\x1b[33m", "\x1b[35m", "\x1b[36m", "\x1b[31m", "\x1b[1m"];
const DIM: &str = "\x1b[2m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

/// Whether to colour and wrap: `CREW_COLOR=always|never`, else `NO_COLOR`, else a terminal on stdout.
pub fn color_enabled() -> bool {
    match std::env::var("CREW_COLOR").as_deref() {
        Ok("always") => true,
        Ok("never") => false,
        _ => std::env::var_os("NO_COLOR").is_none() && std::io::stdout().is_terminal(),
    }
}

/// The terminal's width for wrapping: `COLUMNS`, else 100.
pub fn width() -> usize {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|c| c.parse().ok())
        .filter(|w: &usize| *w >= 40)
        .unwrap_or(100)
}

fn paint(name: &str, roles: &[String], color: bool, prefix: &str, suffix: &str) -> String {
    if !color {
        return format!("{prefix}{name}{suffix}");
    }
    match roles.iter().position(|r| r == name) {
        Some(i) => format!("{}{prefix}{name}{suffix}{RESET}", PALETTE[i % PALETTE.len()]),
        None => format!("{DIM}{prefix}{name}{suffix}{RESET}"),
    }
}

/// A note split for display: (date, time, sender, recipient, text).
fn split(line: &str) -> Option<(&str, &str, &str, Option<&str>, &str)> {
    let (when, role, text) = parse_note(line)?;
    let (date, time) = when.split_once(' ')?;
    let (to, rest) = match text.strip_prefix("to ") {
        Some(r) => match r.split_once(": ") {
            Some((who, rest)) if !who.contains(' ') => (Some(who), rest),
            _ => (None, text),
        },
        None => (None, text),
    };
    Some((date, time, role, to, rest))
}

/// The text with its tag set off: `done:` in bold on a terminal.
fn body(text: &str, color: bool) -> String {
    match tag_of(text) {
        Some(tag) if tag != "to" && color => {
            let rest = text[tag.len() + 1..].trim_start();
            format!("{BOLD}{tag}:{RESET} {rest}")
        }
        _ => text.to_string(),
    }
}

/// One plain line: `2026-10-10 16:39 <operations> @crew the user asks every role`.
pub fn note(line: &str, roles: &[String], color: bool) -> String {
    let Some((date, time, role, to, text)) = split(line) else {
        return line.to_string();
    };
    let when = if color {
        format!("{DIM}{date} {time}{RESET}")
    } else {
        format!("{date} {time}")
    };
    match to {
        Some(who) => format!(
            "{when} {} {} {}",
            paint(role, roles, color, "<", ">"),
            paint(who, roles, color, "@", ""),
            body(text, color)
        ),
        None => format!("{when} {} {}", paint(role, roles, color, "<", ">"), body(text, color)),
    }
}

/// Wrap `text` to `width`, every line after the first indented by `indent` spaces.
fn wrap(text: &str, first_used: usize, indent: usize, width: usize) -> String {
    let mut out = String::new();
    let mut col = first_used;
    let mut first_on_line = true;
    for word in text.split_whitespace() {
        let w = word.chars().count();
        if !first_on_line && col + 1 + w > width {
            out.push('\n');
            out.push_str(&" ".repeat(indent));
            col = indent;
            first_on_line = true;
        }
        if !first_on_line {
            out.push(' ');
            col += 1;
        }
        out.push_str(word);
        col += w;
        first_on_line = false;
    }
    out
}

/// The chat as a chat: dividers between days, time and sender per line, text wrapped under itself.
/// Without colour it is the plain one-line form, so pipes and agents get one note per line.
pub fn chat(lines: &[&str], roles: &[String], color: bool) -> String {
    chat_at(lines, roles, color, width())
}

/// `chat` at a given width, so tests need not touch the environment.
pub fn chat_at(lines: &[&str], roles: &[String], color: bool, width: usize) -> String {
    let mut out = String::new();
    if !color {
        for l in lines {
            out.push_str(&note(l, roles, false));
            out.push('\n');
        }
        return out;
    }
    let mut day = "";
    for l in lines {
        let Some((date, time, role, to, text)) = split(l) else {
            out.push_str(l);
            out.push('\n');
            continue;
        };
        if date != day {
            let rule = "─".repeat(width.saturating_sub(date.len() + 4).min(60));
            out.push_str(&format!("{DIM}── {date} {rule}{RESET}\n"));
            day = date;
        }
        let head_plain = match to {
            Some(who) => format!("{time}  <{role}> @{who}  "),
            None => format!("{time}  <{role}>  "),
        };
        let head = match to {
            Some(who) => format!(
                "{DIM}{time}{RESET}  {} {}  ",
                paint(role, roles, true, "<", ">"),
                paint(who, roles, true, "@", "")
            ),
            None => format!("{DIM}{time}{RESET}  {}  ", paint(role, roles, true, "<", ">")),
        };
        let indent = 7;
        let plain_body = body(text, false);
        let wrapped = wrap(&plain_body, head_plain.chars().count(), indent, width);
        // Set the tag off on the first line only.
        let wrapped = match tag_of(text) {
            Some(tag) if tag != "to" => wrapped.replacen(&format!("{tag}:"), &format!("{BOLD}{tag}:{RESET}"), 1),
            _ => wrapped,
        };
        out.push_str(&head);
        out.push_str(&wrapped);
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roles() -> Vec<String> {
        vec!["operations".to_string(), "crew".to_string()]
    }

    #[test]
    fn plain_lines_read_as_chat() {
        assert_eq!(
            note(
                "- 2026-10-10 16:39 operations: to crew: the user asks every role",
                &roles(),
                false
            ),
            "2026-10-10 16:39 <operations> @crew the user asks every role"
        );
        assert_eq!(
            note("- 2026-10-10 16:40 crew: done: 0.2.1", &roles(), false),
            "2026-10-10 16:40 <crew> done: 0.2.1"
        );
        assert_eq!(
            note("- 2026-10-10 16:40 user: to the point: yes", &roles(), false),
            "2026-10-10 16:40 <user> to the point: yes"
        );
        assert_eq!(note("not a note", &roles(), false), "not a note");
        let c = note("- 2026-10-10 16:39 operations: to crew: x", &roles(), true);
        assert!(
            c.starts_with("\x1b[2m2026-10-10 16:39\x1b[0m \x1b[32m<operations>\x1b[0m \x1b[33m@crew\x1b[0m x"),
            "{c:?}"
        );
        let u = note("- 2026-10-10 16:39 user: x", &roles(), true);
        assert!(u.contains("\x1b[2m<user>\x1b[0m x"), "{u:?}");
        assert!(c.ends_with("x") && u.ends_with(" x"));
        let t = note("- 2026-10-10 16:39 crew: done: shipped", &roles(), true);
        assert!(t.ends_with("\x1b[1mdone:\x1b[0m shipped"), "{t:?}");
    }

    #[test]
    fn the_chat_has_day_dividers_and_wraps() {
        let lines = [
            "- 2026-10-10 16:39 operations: to crew: the user asks every role for a status line now, one line each, by message or by chat note",
            "- 2026-10-10 16:42 crew: done: shipped",
            "- 2026-10-11 09:00 crew: next day",
        ];
        let out = chat_at(&lines, &roles(), true, 60);
        let plain: String = strip(&out);
        assert!(plain.starts_with("── 2026-10-10 "), "{plain}");
        assert!(
            plain.contains("\n16:39  <operations> @crew  the user asks every role for a\n       status line now, one line each, by message or by chat\n       note\n"),
            "{plain}"
        );
        assert!(
            plain.contains("\n16:42  <crew>  done: shipped\n── 2026-10-11 "),
            "{plain}"
        );
        assert!(plain.ends_with("09:00  <crew>  next day\n"), "{plain}");
        assert_eq!(
            chat(&lines[1..2], &roles(), false),
            "2026-10-10 16:42 <crew> done: shipped\n"
        );
        std::env::remove_var("COLUMNS");
    }

    fn strip(s: &str) -> String {
        let mut out = String::new();
        let mut in_esc = false;
        for c in s.chars() {
            if in_esc {
                if c == 'm' {
                    in_esc = false;
                }
            } else if c == '\x1b' {
                in_esc = true;
            } else {
                out.push(c);
            }
        }
        out
    }
}
