//! 0.2.0: the decisions queue, resources, now, adopt, handoff and up.
mod common;
use common::*;
#[allow(unused_imports)]
use predicates::prelude::*;
use std::fs;
use std::path::Path;

fn out(e: &Env, args: &[&str]) -> String {
    String::from_utf8(e.crew().args(args).assert().success().get_output().stdout.clone()).unwrap()
}

#[test]
fn ask_and_answer_go_through_the_chat() {
    let e = Env::new();
    let text = out(
        &e,
        &[
            "ask",
            "--as",
            "engine",
            "Which model for the relay?",
            "--options",
            "haiku,sonnet",
            "--recommend",
            "haiku",
        ],
    );
    assert!(
        text.contains("d1 asked by engine: Which model for the relay?")
            && text.contains("- haiku (recommended)")
            && text.contains("- sonnet"),
        "{text}"
    );
    let chat = e.chat_on_remote();
    assert!(chat.contains("## Decisions pending\n"), "{chat}");
    assert!(
        chat.contains("- d1 · ")
            && chat.contains(" · engine: Which model for the relay?\n  - [ ] haiku (recommended)\n  - [ ] sonnet\n"),
        "{chat}"
    );
    assert!(chat.find("## Decisions pending").unwrap() < chat.find("## Status").unwrap());
    assert_eq!(git(&e.bare, &["log", "-1", "--format=%s", "main"]), "chat: ask engine");

    // Pending in chat, status, hook; a second ask gets d2.
    assert!(
        out(&e, &["chat", "--pending"]).contains("d1 · ")
            && out(&e, &["chat", "--pending"]).contains("answer with `crew answer <id> <option>`")
    );
    assert!(out(&e, &["status"]).contains("1 decision(s) pending"));
    let hook = String::from_utf8(
        e.crew()
            .current_dir(&e.engine)
            .arg("hook")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .unwrap();
    assert!(hook.contains("1 decision(s) waiting for the user"), "{hook}");
    out(&e, &["ask", "Ship tonight?", "--options", "yes", "--options", "no"]);
    assert!(
        out(&e, &["chat", "--pending"]).contains("d2 · ") && out(&e, &["chat", "--pending"]).contains("asked by user")
    );

    // Answers: wrong id, wrong option, then the real one.
    e.crew()
        .args(["answer", "d9", "haiku"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no pending decision d9; open: d1, d2"));
    e.crew()
        .args(["answer", "d1", "opus"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("the options are: haiku, sonnet"));
    let a = out(&e, &["answer", "d1", "Haiku"]);
    assert!(
        a.contains(" user: decision: d1 haiku; Which model for the relay? (asked by engine at "),
        "{a}"
    );
    let chat = e.chat_on_remote();
    assert!(!chat.contains("- d1 · ") && chat.contains("- d2 · "), "{chat}");
    assert!(
        chat.ends_with("decision: d1 haiku; Which model for the relay? (asked by engine at 2026-10-10 00:00)")
            || chat.contains("decision: d1 haiku;"),
        "{chat}"
    );
    let decisions = out(&e, &["chat", "--decisions"]);
    assert_eq!(decisions.lines().count(), 1);
    // Ids never repeat: the next ask is d3.
    out(&e, &["ask", "--as", "engine", "Again?", "--options", "a,b"]);
    assert!(out(&e, &["chat", "--pending"]).contains("d3 · "));
    e.crew()
        .args(["ask", "Only one", "--options", "a"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("at least two options"));
    e.crew()
        .args(["ask", "Bad rec", "--options", "a,b", "--recommend", "c"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("not among the options"));
}

#[test]
fn a_box_ticked_on_the_forge_becomes_a_decision_note_at_the_next_write() {
    let e = Env::new();
    out(&e, &["ask", "--as", "product", "Tick me", "--options", "left,right"]);
    let chat = e.chat_on_remote().replace("  - [ ] right", "  - [x] right");
    e.write_remote_chat(&chat);
    let pending = out(&e, &["chat", "--pending"]);
    assert!(
        pending.contains("answered on the forge: right (recorded as a note at the next chat write)"),
        "{pending}"
    );
    let note = out(&e, &["note", "--as", "engine", "unrelated"]);
    assert!(note.contains("unrelated"));
    let chat = e.chat_on_remote();
    assert!(!chat.contains("## Decisions pending"), "{chat}");
    assert!(chat.contains(" user: decision: d1 right; Tick me (asked by "), "{chat}");
    assert!(chat.ends_with(" engine: unrelated"), "{chat}");
    assert_eq!(out(&e, &["chat", "--pending"]).trim(), "(no decisions pending)");
    // Answering a decision already ticked and recorded is refused with the open list.
    e.crew()
        .args(["answer", "d1", "left"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no pending decision d1; open: none"));
}

#[test]
fn resources_live_on_the_chat_and_in_status() {
    let e = Env::new();
    assert!(out(&e, &["resource"]).contains("(no resources declared"));
    let r = out(
        &e,
        &[
            "resource",
            "--as",
            "engine",
            "second-puffin:18080",
            "restarts only at commits, announced first",
        ],
    );
    assert!(
        r.contains("second-puffin:18080 (engine): restarts only at commits, announced first"),
        "{r}"
    );
    let chat = e.chat_on_remote();
    assert!(chat.contains("## Resources\n\n| resource | owner | rules |\n| --- | --- | --- |\n| second-puffin:18080 | engine | restarts only at commits, announced first |\n"), "{chat}");
    assert!(out(
        &e,
        &[
            "resource",
            "--as",
            "engine",
            "second-puffin:18080",
            "restarts only at commits, announced first"
        ]
    )
    .contains("already says that"));
    out(
        &e,
        &[
            "resource",
            "--as",
            "operations",
            "m0-prod8000",
            "ask | operations first",
        ],
    );
    let list = out(&e, &["resource"]);
    assert!(
        list.contains("second-puffin:18080 (engine)")
            && list.contains("m0-prod8000 (operations): ask | operations first"),
        "{list}"
    );
    assert!(out(&e, &["status"]).contains("resources: second-puffin:18080 (engine): restarts only at commits, announced first; m0-prod8000 (operations): ask | operations first"));
    // A later note keeps the table, and a sync keeps it too.
    out(&e, &["note", "--as", "operations", "after"]);
    out(&e, &["chat", "--sync-roles"]);
    let chat = e.chat_on_remote();
    assert!(
        chat.contains("| m0-prod8000 | operations | ask \\| operations first |")
            && chat.ends_with(" operations: after"),
        "{chat}"
    );
    e.crew()
        .args(["resource", "x"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("needs a name and one line of rules"));
}

#[test]
fn now_is_a_local_claim_shown_by_status_and_counted_by_doctor() {
    let e = Env::new();
    e.crew().arg("install-hook").assert().success();
    assert!(out(&e, &["now"]).lines().all(|l| l.ends_with('-')));
    let n = out(&e, &["now", "--as", "engine", "measuring the", "draft block"]);
    assert_eq!(n.trim(), "engine now: measuring the draft block");
    let text = fs::read_to_string(e.home.join(".cache/mighty-crew/now/engine")).unwrap();
    assert!(text.lines().nth(1) == Some("measuring the draft block"), "{text}");
    assert!(out(&e, &["now"]).contains("engine       20") && out(&e, &["now"]).contains("measuring the draft block"));
    let status = out(&e, &["status"]);
    let engine = status.lines().find(|l| l.starts_with("engine")).unwrap();
    assert!(engine.contains("measuring the draft block (0m ago)"), "{engine}");
    assert!(status.lines().next().unwrap().contains("now"));
    // A busy role with an old commit and no note is not quiet while its now line is fresh.
    git_env(
        &e.engine,
        &["commit", "-q", "--allow-empty", "-m", "old"],
        &[("GIT_COMMITTER_DATE", "2020-01-01T00:00:00+00:00")],
    );
    e.crew().args(["start", "engine"]).assert().success();
    e.sessions(&session_json(&e.engine, "engine", "busy"));
    e.crew()
        .arg("doctor")
        .assert()
        .stdout(predicates::str::contains("warn role engine").not());
    e.crew()
        .args(["now", "x"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("in no role's tree"));
}

#[test]
fn adopt_records_a_hand_started_session() {
    let e = Env::new();
    e.crew().arg("install-hook").assert().success();
    e.crew()
        .args(["adopt", "engine"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no session is running"));
    e.sessions(&session_json(&e.engine, "engine-1a", "busy"));
    e.crew()
        .arg("doctor")
        .assert()
        .code(1)
        .stdout(predicates::str::contains("not started by `crew start`"));
    e.crew()
        .args(["adopt", "engine"])
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "adopted: role engine runs session engine-1a in bypassPermissions",
        ));
    e.crew()
        .arg("doctor")
        .assert()
        .code(0)
        .stdout(predicates::str::contains(
            "session engine-1a (busy) started in bypassPermissions",
        ));
    e.crew()
        .args(["adopt", "engine", "--mode", "acceptEdits"])
        .assert()
        .success()
        .stderr(predicates::str::contains("not the crew's class"));
    e.crew()
        .arg("doctor")
        .assert()
        .code(1)
        .stdout(predicates::str::contains(
            "started in acceptEdits, the configuration now says bypassPermissions",
        ));
}

#[test]
fn handoff_copies_into_the_neutral_folder_and_notes_it() {
    let e = Env::new();
    let src = e.root.join("recordings");
    fs::create_dir_all(src.join("inner")).unwrap();
    fs::write(src.join("a.txt"), "clean\n").unwrap();
    fs::write(src.join("inner/b.log"), "captured at /tmp/xyz/run\n").unwrap();
    let text = String::from_utf8(
        e.crew()
            .current_dir(&e.product)
            .args(["handoff", "engine", src.to_str().unwrap()])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .unwrap();
    let dest = e.home.join(".cache/mighty-crew/handoff/product/engine/recordings");
    assert!(
        dest.join("a.txt").is_file() && dest.join("inner/b.log").is_file(),
        "{text}"
    );
    assert!(
        text.contains(" product: to engine: handoff recordings at ") && text.contains("(2 files)"),
        "{text}"
    );
    assert!(text.contains("1 file(s) mention /tmp/ paths"), "{text}");
    assert!(e.chat_on_remote().contains("to engine: handoff recordings at"));
    e.crew()
        .current_dir(&e.product)
        .args(["handoff", "engine", src.to_str().unwrap()])
        .assert()
        .failure()
        .stderr(predicates::str::contains("exists already"));
    e.crew()
        .args(["handoff", "engine", "/nonexistent/thing"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("does not exist"));
    // A single file, from outside any tree, is signed `user`.
    let one = String::from_utf8(
        e.crew()
            .args(["handoff", "product", src.join("a.txt").to_str().unwrap()])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .unwrap();
    assert!(
        one.contains(" user: to product: handoff a.txt at ")
            && one.contains("(1 files)")
            && one.contains("scrub temporary paths"),
        "{one}"
    );
}

fn tmux_stub(dir: &Path) -> std::path::PathBuf {
    fs::create_dir_all(dir).unwrap();
    let p = dir.join("tmux");
    fs::write(
        &p,
        "#!/bin/bash\nprintf '%s\\n' \"$*\" >> \"$STUB_TMUX_LOG\"\nif [ \"$1\" = has-session ]; then [ -n \"$STUB_TMUX_HAS\" ]; exit $?; fi\nexit 0\n",
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    p
}

#[test]
fn up_opens_every_role_in_tmux_or_prints_the_lines() {
    let e = Env::new();
    // Without tmux or a display: the lines to run, and nothing started. PATH holds git alone.
    let minbin = e.root.join("minbin");
    fs::create_dir_all(&minbin).unwrap();
    let git_path = String::from_utf8(
        std::process::Command::new("sh")
            .args(["-c", "command -v git"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    std::os::unix::fs::symlink(git_path.trim(), minbin.join("git")).unwrap();
    // The stub agent needs cat; nothing else from the system is on this PATH, so no real tmux.
    let cat_path = String::from_utf8(
        std::process::Command::new("sh")
            .args(["-c", "command -v cat"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    std::os::unix::fs::symlink(cat_path.trim(), minbin.join("cat")).unwrap();
    let text = String::from_utf8(
        e.crew()
            .env("PATH", &minbin)
            .arg("up")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .unwrap();
    assert!(
        text.contains("no tmux and no display") && text.contains("start operations") && text.contains("start engine"),
        "{text}"
    );
    assert!(e.stub_args().is_empty() || e.stub_args()[0] == "agents");

    // A missing configured worktree is created; a live role is skipped; tmux gets one window per role.
    let tree = e.ops.join("wt/other");
    e.write_config(
        &e.default_config()
            .replace("branch = \"work/product\"", "branch = \"work/other\"")
            .replace(e.product.to_str().unwrap(), tree.to_str().unwrap()),
    );
    e.sessions(&session_json(&e.engine, "engine", "busy"));
    let bin = e.root.join("tmuxbin");
    tmux_stub(&bin);
    let log = e.root.join("tmux.log");
    let path = format!("{}:{}", bin.display(), minbin.display());
    let text = String::from_utf8(
        e.crew()
            .env("PATH", &path)
            .env("STUB_TMUX_LOG", &log)
            .args(["up", "--new"])
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .unwrap();
    assert!(tree.is_dir(), "{text}");
    assert!(
        text.contains("engine: already running (engine)")
            && text.contains("operations: started in tmux window crew-test:operations")
            && text.contains("product: started in tmux window crew-test:product"),
        "{text}"
    );
    let calls = fs::read_to_string(&log).unwrap();
    let lines: Vec<&str> = calls.lines().collect();
    assert_eq!(lines[0], "has-session -t crew-test");
    assert!(
        lines[1].starts_with("new-session -d -s crew-test -n operations -e MIGHTY_CREW_CONFIG=")
            && lines[1].ends_with(" start operations --new"),
        "{calls}"
    );
    assert!(
        lines[2].starts_with("new-window -t crew-test -n product -e MIGHTY_CREW_CONFIG=")
            && lines[2].ends_with(" start product --new"),
        "{calls}"
    );
    assert_eq!(lines[3], "attach -t crew-test");
    assert!(!calls.contains("engine"), "{calls}");

    // An existing tmux session is attached, not rebuilt.
    fs::remove_file(&log).unwrap();
    e.crew()
        .env("PATH", &path)
        .env("STUB_TMUX_LOG", &log)
        .env("STUB_TMUX_HAS", "1")
        .arg("up")
        .assert()
        .success()
        .stdout(predicates::str::contains("tmux session crew-test exists; attaching"));
    assert_eq!(
        fs::read_to_string(&log).unwrap().trim(),
        "has-session -t crew-test\nattach -t crew-test"
    );

    // Inside tmux already: switch-client.
    fs::remove_file(&log).unwrap();
    e.crew()
        .env("PATH", &path)
        .env("STUB_TMUX_LOG", &log)
        .env("STUB_TMUX_HAS", "1")
        .env("TMUX", "/tmp/tmux-1/default,1,0")
        .arg("up")
        .assert()
        .success();
    assert!(fs::read_to_string(&log)
        .unwrap()
        .ends_with("switch-client -t crew-test\n"));

    // A role whose tree is missing and not configured is reported, and the rest still opens.
    e.write_config(
        &e.default_config()
            .replace(e.engine.to_str().unwrap(), e.root.join("gone").to_str().unwrap()),
    );
    e.sessions("[]");
    fs::remove_file(&log).unwrap();
    e.crew()
        .env("PATH", &path)
        .env("STUB_TMUX_LOG", &log)
        .arg("up")
        .assert()
        .stderr(predicates::str::contains("does not exist"))
        .stdout(predicates::str::contains("operations: started in tmux window"));
}
