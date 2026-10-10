//! The chat of §4: the header, the clone as a mirror, contention, an unreachable remote.
#![cfg(unix)]
mod common;
use common::*;
#[allow(unused_imports)]
use predicates::prelude::*;
use std::fs;

#[test]
fn sync_roles_rewrites_the_header_and_keeps_every_note() {
    let e = Env::new();
    e.crew().args(["note", "--as", "engine", "kept"]).assert().success();
    e.crew()
        .args(["chat", "--sync-roles"])
        .assert()
        .success()
        .stdout(predicates::str::contains("role table rewritten"));
    let chat = e.chat_on_remote();
    assert!(chat.starts_with("# test: the crew's chat\n"), "{chat}");
    assert!(
        chat.contains(&format!("| operations | `{}` | the machine |", e.ops.to_str().unwrap())),
        "{chat}"
    );
    assert!(
        chat.contains("| product | `") && chat.contains("| engine | `"),
        "{chat}"
    );
    assert!(
        chat.contains("## Status\n\n- 2026-10-10 09:00 operations: seed line\n") && chat.ends_with(" engine: kept"),
        "{chat}"
    );
    assert_eq!(git(&e.bare, &["log", "-1", "--format=%s", "main"]), "chat: roles");
    e.crew()
        .args(["chat", "--sync-roles"])
        .assert()
        .success()
        .stdout(predicates::str::contains("already current"));
    assert_eq!(git(&e.bare, &["log", "-1", "--format=%s", "main"]), "chat: roles");
}

#[test]
fn chat_counts_lines_and_survives_an_unreachable_remote() {
    let e = Env::new();
    e.crew().args(["note", "--as", "engine", "one"]).assert().success();
    e.crew().args(["note", "--as", "product", "two"]).assert().success();
    let one = String::from_utf8(
        e.crew()
            .args(["chat", "-n", "1"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .unwrap();
    assert_eq!(one.lines().count(), 1);
    assert!(one.contains("<product> two"));
    let two = String::from_utf8(
        e.crew()
            .args(["chat", "-n", "2"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .unwrap();
    assert_eq!(two.lines().count(), 2);
    assert!(two.starts_with("2026-") && two.contains("<engine> one\n"));
    let all = String::from_utf8(
        e.crew()
            .args(["chat", "--all"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .unwrap();
    assert_eq!(all.lines().count(), 3);

    // The remote disappears: a fetch fails with one line, the cached copy still prints, a note fails.
    let away = e.root.join("away.git");
    fs::rename(&e.bare, &away).unwrap();
    e.crew()
        .arg("chat")
        .assert()
        .failure()
        .stderr(predicates::str::starts_with("crew: cannot fetch the chat"));
    e.crew()
        .args(["chat", "--cached"])
        .assert()
        .success()
        .stdout(predicates::str::contains("<product> two"));
    e.crew()
        .args(["note", "--as", "engine", "lost"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("cannot fetch the chat"));
    fs::rename(&away, &e.bare).unwrap();
    e.crew().args(["note", "--as", "engine", "back"]).assert().success();
    assert!(e.chat_on_remote().ends_with(" engine: back"));
}

#[test]
fn note_text_is_one_line_and_never_empty() {
    let e = Env::new();
    e.crew()
        .current_dir(&e.engine)
        .args(["note", "first\n   second\tthird  "])
        .assert()
        .success()
        .stdout(predicates::str::ends_with(" engine: first second third\n"));
    e.crew()
        .args(["note", "--as", "engine", "   "])
        .assert()
        .failure()
        .stderr(predicates::str::contains("the note is empty"));
    e.crew()
        .args(["note", "--as", "nobody", "x"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no role named nobody"));
}

#[test]
fn three_roles_writing_at_once_all_land() {
    let e = Env::new();
    let roles = ["operations", "product", "engine"];
    let caches: Vec<_> = roles.iter().map(|r| e.root.join(format!("cache-{r}"))).collect();
    for c in &caches {
        e.crew().env("XDG_CACHE_HOME", c).arg("chat").assert().success();
    }
    let children: Vec<_> = roles
        .iter()
        .zip(&caches)
        .map(|(r, c)| {
            e.crew_std()
                .env("XDG_CACHE_HOME", c)
                .args(["note", "--as", r, &format!("{r} at once")])
                .spawn()
                .unwrap()
        })
        .collect();
    for mut ch in children {
        assert!(ch.wait().unwrap().success());
    }
    let lines = e.remote_lines();
    for r in roles {
        assert!(
            lines.iter().any(|l| l.ends_with(&format!(" {r}: {r} at once"))),
            "{lines:?}"
        );
    }
    assert_eq!(lines.len(), 4);
    assert_eq!(git(&e.bare, &["rev-list", "--count", "main"]), "5");
}

#[test]
fn the_clone_is_a_mirror_of_the_remote() {
    let e = Env::new();
    e.crew().arg("chat").assert().success();
    let clone = e.home.join(".cache/mighty-crew/chat/chat");
    assert!(clone.join(".git").is_dir(), "clone at {}", clone.display());
    // Junk in the clone, committed or not, never reaches the remote.
    fs::write(clone.join("docs/dev/sessions.md"), "junk\n").unwrap();
    git(&clone, &["commit", "-q", "-am", "junk"]);
    e.crew().args(["note", "--as", "engine", "clean"]).assert().success();
    let chat = e.chat_on_remote();
    assert!(!chat.contains("junk") && chat.ends_with(" engine: clean"), "{chat}");
    assert_eq!(git(&e.bare, &["rev-list", "--count", "main"]), "3");
    // A deleted clone is rebuilt.
    fs::remove_dir_all(&clone).unwrap();
    e.crew()
        .arg("chat")
        .assert()
        .success()
        .stdout(predicates::str::contains("<engine> clean"));
    assert!(clone.join(".git").is_dir());
}

#[test]
fn chat_for_a_role_includes_lines_addressed_to_it() {
    let e = Env::new();
    e.crew().args(["note", "--as", "engine", "mine"]).assert().success();
    e.crew()
        .args(["note", "--as", "product", "to engine: for you"])
        .assert()
        .success();
    e.crew()
        .args(["note", "--as", "product", "not for engine"])
        .assert()
        .success();
    let out = String::from_utf8(
        e.crew()
            .args(["chat", "--for", "engine", "--all"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .unwrap();
    assert_eq!(out.lines().count(), 2, "{out}");
    assert!(
        out.contains("<engine> mine") && out.contains("<product> @engine for you") && !out.contains("not for"),
        "{out}"
    );
    let last = String::from_utf8(
        e.crew()
            .args(["chat", "--for", "engine", "-n", "1"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .unwrap();
    assert_eq!(last.lines().count(), 1);
    assert!(last.contains("for you"));
    e.crew()
        .args(["chat", "--for", "nobody"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no role named nobody"));
}

#[test]
fn a_hand_edited_chat_without_the_heading_gets_one() {
    let e = Env::new();
    e.write_remote_chat("just some text\n");
    e.crew().args(["note", "--as", "engine", "after"]).assert().success();
    let chat = e.chat_on_remote();
    assert!(
        chat.starts_with("# test: the crew's chat") && chat.contains("## Status") && chat.ends_with(" engine: after"),
        "{chat}"
    );
    assert!(!chat.contains("just some text"));
}
