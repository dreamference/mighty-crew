//! The first day's lessons that became code: relay sessions, tagged notes, activity from git,
//! the hook's messaging rule.
#![cfg(unix)]
mod common;
use common::*;
#[allow(unused_imports)]
use predicates::prelude::*;
use std::fs;

#[test]
fn a_relay_session_is_never_a_role_and_is_labelled() {
    let e = Env::new();
    e.crew().arg("install-hook").assert().success();
    e.sessions(&format!(
        "[{},{}]",
        session_json(&e.product, "crew-msg", "busy").trim_matches(|c| c == '[' || c == ']'),
        session_json(&e.root, "crew-msg", "busy").trim_matches(|c| c == '[' || c == ']')
    ));
    let out = String::from_utf8(e.crew().arg("status").assert().success().get_output().stdout.clone()).unwrap();
    assert!(
        out.lines()
            .find(|l| l.starts_with("product"))
            .unwrap()
            .contains("not running"),
        "{out}"
    );
    let doc = String::from_utf8(e.crew().arg("doctor").assert().code(0).get_output().stdout.clone()).unwrap();
    assert!(
        doc.contains("ok   2 relay session(s) of `crew msg` in flight (crew-msg), not roles"),
        "{doc}"
    );
    assert!(!doc.contains("no role's tree"), "{doc}");
    // start in the product tree is not refused by a relay sitting there.
    e.crew().args(["start", "product"]).assert().success();
}

#[test]
fn tagged_notes_filter_on_the_chat() {
    let e = Env::new();
    e.crew()
        .args(["note", "--as", "engine", "done: custom tools landed"])
        .assert()
        .success();
    e.crew()
        .args(["note", "--as", "operations", "decision: both machines stay on"])
        .assert()
        .success();
    e.crew()
        .args(["note", "--as", "product", "to engine: run against 18080"])
        .assert()
        .success();
    e.crew()
        .args(["note", "--as", "product", "heads-up: restarting the server at 17:00"])
        .assert()
        .success();
    e.crew()
        .args(["note", "--as", "product", "plain text with a colon: later"])
        .assert()
        .success();
    let out =
        |args: &[&str]| String::from_utf8(e.crew().args(args).assert().success().get_output().stdout.clone()).unwrap();
    let done = out(&["chat", "--tag", "done"]);
    assert_eq!(done.lines().count(), 1);
    assert!(done.contains("<engine> done: custom tools landed"));
    assert_eq!(
        out(&["chat", "--decisions"]).trim(),
        out(&["chat", "--tag", "decision:"]).trim()
    );
    assert!(out(&["chat", "--decisions"]).contains("both machines stay on"));
    assert_eq!(out(&["chat", "--tag", "to"]).lines().count(), 1);
    assert_eq!(out(&["chat", "--tag", "heads-up"]).lines().count(), 1);
    assert_eq!(out(&["chat", "--tag", "plain"]).trim(), "(no matching notes)");
    assert_eq!(
        out(&["chat", "--for", "engine", "--tag", "done", "--all"])
            .lines()
            .count(),
        1
    );
    assert_eq!(
        out(&["chat", "--for", "engine", "--tag", "decision"]).trim(),
        "(no matching notes)"
    );
    e.crew().args(["chat", "--tag", "x", "--decisions"]).assert().code(2);
}

#[test]
fn status_reads_activity_from_git_and_doctor_warns_on_a_quiet_busy_role() {
    let e = Env::new();
    e.crew().arg("install-hook").assert().success();
    e.crew()
        .current_dir(&e.engine)
        .args(["note", "starting"])
        .assert()
        .success();
    fs::write(e.engine.join("work"), "x\n").unwrap();
    git(&e.engine, &["add", "."]);
    git(&e.engine, &["commit", "-q", "-m", "after the note"]);
    let out = String::from_utf8(e.crew().arg("status").assert().success().get_output().stdout.clone()).unwrap();
    let engine = out.lines().find(|l| l.starts_with("engine")).unwrap();
    // The note's time is a whole minute, so the tree's first commit from the same minute counts too.
    assert!(
        predicates::str::is_match(r"[12] commits? since note 0m ago, commit 0m ago")
            .unwrap()
            .eval(engine),
        "{engine}"
    );
    let ops = out.lines().find(|l| l.starts_with("operations")).unwrap();
    assert!(ops.contains("ago, commit 0m ago") && ops.contains("note"), "{ops}");
    assert!(out.lines().next().unwrap().contains("activity"), "{out}");

    // Operations: busy, its last note is the seed line of 09:00 on 2026-10-10, its last commit backdated.
    git_env(
        &e.ops,
        &["commit", "-q", "--allow-empty", "-m", "old"],
        &[("GIT_COMMITTER_DATE", "2020-01-01T00:00:00+00:00")],
    );
    e.crew().args(["start", "operations"]).assert().success();
    e.sessions(&session_json(&e.ops, "operations", "busy"));
    e.crew()
        .arg("doctor")
        .assert()
        .code(0)
        .stdout(predicates::str::contains(
            "warn role operations: its session is busy with neither a note nor a commit in 90 minutes",
        ));
    // A fresh note clears it.
    e.crew()
        .current_dir(&e.ops)
        .args(["note", "doing: nights"])
        .assert()
        .success();
    e.crew()
        .arg("doctor")
        .assert()
        .code(0)
        .stdout(predicates::str::contains("warn role operations").not());
    // An idle session is never warned about.
    git_env(
        &e.engine,
        &["commit", "-q", "--allow-empty", "-m", "old"],
        &[("GIT_COMMITTER_DATE", "2020-01-01T00:00:00+00:00")],
    );
    e.sessions(&session_json(&e.engine, "engine", "idle"));
    e.crew()
        .arg("doctor")
        .assert()
        .stdout(predicates::str::contains("warn role engine").not());
}

#[test]
fn the_hook_states_the_messaging_rule() {
    let e = Env::new();
    e.crew().current_dir(&e.engine).arg("hook").assert().success().stdout(
        predicates::str::contains("goes through `crew msg <role> \"<text>\"`")
            .and(predicates::str::contains("SendMessage alone leaves no record")),
    );
}
