//! The chat as sections: the head (title, preamble, role table), `## Resources`,
//! `## Decisions pending`, and `## Status` with the notes. Pure text in, text out.

use crate::util;

pub const RESOURCES: &str = "## Resources";
pub const PENDING: &str = "## Decisions pending";
pub const STATUS: &str = "## Status";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decision {
    pub id: String,
    pub when: String,
    pub role: String,
    pub question: String,
    pub options: Vec<String>,
    pub recommended: Option<String>,
    /// The option ticked on the forge, if any.
    pub ticked: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resource {
    pub name: String,
    pub owner: String,
    pub rules: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Doc {
    pub head: String,
    pub resources: Vec<Resource>,
    pub pending: Vec<Decision>,
    /// The status section's lines, as they are (notes and anything else), without blank lines.
    pub notes: Vec<String>,
}

impl Doc {
    /// Split a chat file; a file without `## Status` gives a Doc whose head is the whole text.
    pub fn parse(content: &str) -> Doc {
        let mut doc = Doc::default();
        let mut head = String::new();
        let mut current: Option<&str> = None;
        let mut resources_text = String::new();
        let mut pending_text = String::new();
        for line in content.lines() {
            let t = line.trim_end();
            if t == RESOURCES || t == PENDING || t == STATUS {
                current = Some(if t == RESOURCES {
                    RESOURCES
                } else if t == PENDING {
                    PENDING
                } else {
                    STATUS
                });
                continue;
            }
            match current {
                None => {
                    head.push_str(line);
                    head.push('\n');
                }
                Some(RESOURCES) => {
                    resources_text.push_str(line);
                    resources_text.push('\n');
                }
                Some(PENDING) => {
                    pending_text.push_str(line);
                    pending_text.push('\n');
                }
                Some(_) => {
                    if !t.trim().is_empty() {
                        doc.notes.push(t.to_string());
                    }
                }
            }
        }
        doc.head = head;
        doc.resources = parse_resources(&resources_text);
        doc.pending = parse_decisions(&pending_text);
        doc
    }

    pub fn render(&self) -> String {
        let mut s = self.head.trim_end().to_string();
        s.push_str("\n\n");
        if !self.resources.is_empty() {
            s.push_str(RESOURCES);
            s.push_str("\n\n| resource | owner | rules |\n| --- | --- | --- |\n");
            for r in &self.resources {
                s.push_str(&format!(
                    "| {} | {} | {} |\n",
                    cell(&r.name),
                    cell(&r.owner),
                    cell(&r.rules)
                ));
            }
            s.push('\n');
        }
        if !self.pending.is_empty() {
            s.push_str(PENDING);
            s.push_str("\n\nTick one box on the forge, or run `crew answer <id> <option>`; the next chat write records the choice as a `decision:` note.\n\n");
            for d in &self.pending {
                s.push_str(&render_decision(d));
            }
            s.push('\n');
        }
        s.push_str(STATUS);
        s.push_str("\n\n");
        for n in &self.notes {
            s.push_str(n);
            s.push('\n');
        }
        s
    }

    /// The next decision id: one past the highest `d<N>` seen pending or decided.
    pub fn next_id(&self) -> String {
        let mut max = 0u64;
        for d in &self.pending {
            if let Some(n) = d.id.strip_prefix('d').and_then(|n| n.parse::<u64>().ok()) {
                max = max.max(n);
            }
        }
        for n in &self.notes {
            if let Some((_, _, text)) = crate::chat::parse_note(n) {
                if let Some(rest) = text.strip_prefix("decision: ") {
                    if let Some(id) = rest.split_whitespace().next() {
                        if let Some(n) = id.strip_prefix('d').and_then(|n| n.parse::<u64>().ok()) {
                            max = max.max(n);
                        }
                    }
                }
            }
        }
        format!("d{}", max + 1)
    }

    /// Turn every ticked decision into a `decision:` note by `answered_by`; returns the notes added.
    pub fn resolve_ticks(&mut self, answered_by: &str) -> Vec<String> {
        let mut added = Vec::new();
        let mut keep = Vec::new();
        for d in std::mem::take(&mut self.pending) {
            match &d.ticked {
                Some(opt) => {
                    let line = decision_note(answered_by, &d, opt);
                    self.notes.push(line.clone());
                    added.push(line);
                }
                None => keep.push(d),
            }
        }
        self.pending = keep;
        added
    }

    pub fn upsert_resource(&mut self, name: &str, owner: &str, rules: &str) -> bool {
        if let Some(r) = self.resources.iter_mut().find(|r| r.name == name) {
            let changed = r.owner != owner || r.rules != rules;
            r.owner = owner.to_string();
            r.rules = rules.to_string();
            return changed;
        }
        self.resources.push(Resource {
            name: name.into(),
            owner: owner.into(),
            rules: rules.into(),
        });
        true
    }
}

/// `- <now> <role>: decision: d3 haiku; <question> (asked by <role>)`
pub fn decision_note(by: &str, d: &Decision, option: &str) -> String {
    crate::chat::note_line(
        by,
        &format!(
            "decision: {} {}; {} (asked by {} at {})",
            d.id, option, d.question, d.role, d.when
        ),
    )
}

fn cell(s: &str) -> String {
    util::one_line(s).replace('|', "\\|")
}

fn render_decision(d: &Decision) -> String {
    let mut s = format!(
        "- {} · {} · {}: {}\n",
        d.id,
        d.when,
        d.role,
        util::one_line(&d.question)
    );
    for o in &d.options {
        let mark = if d.ticked.as_deref() == Some(o.as_str()) {
            "x"
        } else {
            " "
        };
        let rec = if d.recommended.as_deref() == Some(o.as_str()) {
            " (recommended)"
        } else {
            ""
        };
        s.push_str(&format!("  - [{mark}] {o}{rec}\n"));
    }
    s
}

fn parse_decisions(text: &str) -> Vec<Decision> {
    let mut out: Vec<Decision> = Vec::new();
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("- d") {
            // "3 · 2026-10-10 16:50 · crew: question"
            let mut parts = rest.splitn(3, " · ");
            let (Some(num), Some(when), Some(rest)) = (parts.next(), parts.next(), parts.next()) else {
                continue;
            };
            let Some((role, question)) = rest.split_once(": ") else {
                continue;
            };
            out.push(Decision {
                id: format!("d{num}"),
                when: when.to_string(),
                role: role.to_string(),
                question: question.to_string(),
                options: Vec::new(),
                recommended: None,
                ticked: None,
            });
        } else if let Some(d) = out.last_mut() {
            let t = line.trim_start();
            let (mark, opt) = if let Some(o) = t.strip_prefix("- [ ] ") {
                (false, o)
            } else if let Some(o) = t.strip_prefix("- [x] ").or_else(|| t.strip_prefix("- [X] ")) {
                (true, o)
            } else {
                continue;
            };
            let (opt, rec) = match opt.strip_suffix(" (recommended)") {
                Some(o) => (o, true),
                None => (opt, false),
            };
            let opt = opt.trim().to_string();
            if rec {
                d.recommended = Some(opt.clone());
            }
            if mark && d.ticked.is_none() {
                d.ticked = Some(opt.clone());
            }
            d.options.push(opt);
        }
    }
    out
}

fn parse_resources(text: &str) -> Vec<Resource> {
    let mut out = Vec::new();
    for line in text.lines() {
        let t = line.trim();
        if !t.starts_with('|') {
            continue;
        }
        let cells: Vec<String> = t
            .trim_matches('|')
            .split(" | ")
            .map(|c| c.trim().replace("\\|", "|"))
            .collect();
        if cells.len() < 3 || cells[0] == "resource" || cells[0].starts_with("---") {
            continue;
        }
        out.push(Resource {
            name: cells[0].clone(),
            owner: cells[1].clone(),
            rules: cells[2..].join(" | "),
        });
    }
    out
}

/// A forge link to the pending section, for http(s) repositories on GitHub-like forges.
pub fn forge_link(repo: &str, branch: &str, path: &str) -> Option<String> {
    let base = repo.strip_suffix(".git").unwrap_or(repo).trim_end_matches('/');
    if !(base.starts_with("https://") || base.starts_with("http://")) {
        return None;
    }
    Some(format!("{base}/blob/{branch}/{path}#decisions-pending"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHAT: &str = "# t: the crew's chat\n\npreamble\n\n| role | working tree | owns |\n| --- | --- | --- |\n| a | `/a` | x |\n\n## Resources\n\n| resource | owner | rules |\n| --- | --- | --- |\n| m0 | operations | ask first \\| then wait |\n\n## Decisions pending\n\nTick one box.\n\n- d2 · 2026-10-10 16:50 · crew: Which model?\n  - [ ] haiku (recommended)\n  - [x] sonnet\n- d5 · 2026-10-10 16:51 · product: Ship?\n  - [ ] yes\n  - [ ] no\n\n## Status\n\n- 2026-10-10 09:00 a: seed\n- 2026-10-10 09:01 user: decision: d4 yes; Older? (asked by a at 2026-10-10 08:00)\n";

    #[test]
    fn parses_every_section() {
        let d = Doc::parse(CHAT);
        assert!(d.head.starts_with("# t: the crew's chat\n") && d.head.contains("| a | `/a` | x |"));
        assert_eq!(
            d.resources,
            vec![Resource {
                name: "m0".into(),
                owner: "operations".into(),
                rules: "ask first | then wait".into()
            }]
        );
        assert_eq!(d.pending.len(), 2);
        assert_eq!(d.pending[0].id, "d2");
        assert_eq!(d.pending[0].options, vec!["haiku", "sonnet"]);
        assert_eq!(d.pending[0].recommended.as_deref(), Some("haiku"));
        assert_eq!(d.pending[0].ticked.as_deref(), Some("sonnet"));
        assert_eq!(d.pending[1].ticked, None);
        assert_eq!(d.notes.len(), 2);
        assert_eq!(d.next_id(), "d6");
    }

    #[test]
    fn renders_back_the_same_shape() {
        let d = Doc::parse(CHAT);
        let again = Doc::parse(&d.render());
        assert_eq!(d, again);
        assert!(d.render().contains("## Resources\n\n| resource | owner | rules |\n| --- | --- | --- |\n| m0 | operations | ask first \\| then wait |\n"));
        assert!(d.render().ends_with("## Status\n\n- 2026-10-10 09:00 a: seed\n- 2026-10-10 09:01 user: decision: d4 yes; Older? (asked by a at 2026-10-10 08:00)\n"));
    }

    #[test]
    fn ticks_become_notes() {
        let mut d = Doc::parse(CHAT);
        let added = d.resolve_ticks("user");
        assert_eq!(added.len(), 1);
        assert!(
            added[0].contains(" user: decision: d2 sonnet; Which model? (asked by crew at 2026-10-10 16:50)"),
            "{}",
            added[0]
        );
        assert_eq!(d.pending.len(), 1);
        assert_eq!(d.notes.len(), 3);
        assert!(d.resolve_ticks("user").is_empty());
    }

    #[test]
    fn plain_chat_has_no_sections() {
        let d = Doc::parse("# x\n\n## Status\n\n- 2026-10-10 09:00 a: seed\n");
        assert!(d.resources.is_empty() && d.pending.is_empty());
        assert_eq!(d.next_id(), "d1");
        assert_eq!(d.render(), "# x\n\n## Status\n\n- 2026-10-10 09:00 a: seed\n");
        let mut d = d;
        assert!(d.upsert_resource("m0", "ops", "rule"));
        assert!(!d.upsert_resource("m0", "ops", "rule"));
        assert!(d.upsert_resource("m0", "ops", "rule 2"));
        assert!(d.render().contains("## Resources"));
    }

    #[test]
    fn forge_links_only_for_http() {
        assert_eq!(
            forge_link("https://github.com/o/r.git", "main", "docs/dev/sessions.md").as_deref(),
            Some("https://github.com/o/r/blob/main/docs/dev/sessions.md#decisions-pending")
        );
        assert_eq!(forge_link("/tmp/chat.git", "main", "x.md"), None);
        assert_eq!(forge_link("git@github.com:o/r.git", "main", "x.md"), None);
    }
}
