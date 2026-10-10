//! doctor, the hook and the skill, init, and the command line's exit codes.
#![cfg(unix)]
mod common;
use common::*;
#[allow(unused_imports)]
use predicates::prelude::*;
use std::fs;

#[test]
fn doctor_exit_codes_and_the_agent_and_terminal_checks() {
    let e = Env::new();
    e.crew().arg("install-hook").assert().success();
    e.crew()
        .arg("doctor")
        .assert()
        .code(0)
        .stdout(predicates::str::contains("all good"));

    e.write_config(
        &e.default_config()
            .replace(e.stub.to_str().unwrap(), "/nonexistent/claude"),
    );
    let out = String::from_utf8(e.crew().arg("doctor").assert().code(1).get_output().stdout.clone()).unwrap();
    assert!(
        out.contains("fix  agent command /nonexistent/claude not found") && out.contains("fix  session listing:"),
        "{out}"
    );

    e.write_config(&(e.default_config() + "\n[terminal]\ncommands = [\"/nonexistent/term\"]\n"));
    e.crew()
        .arg("doctor")
        .assert()
        .code(1)
        .stdout(predicates::str::contains("fix  no terminal among /nonexistent/term"));

    e.write_config("not toml at all [[[");
    e.crew()
        .arg("doctor")
        .assert()
        .code(1)
        .stdout(predicates::str::starts_with("fix  configuration"));
}

#[test]
fn doctor_warns_on_out_of_order_notes_without_failing() {
    let e = Env::new();
    e.crew().arg("install-hook").assert().success();
    e.write_remote_chat(&(CHAT_SEED.to_string() + "- 2026-10-10 08:00 engine: typed by hand\n"));
    e.crew().arg("doctor").assert().code(0).stdout(
        predicates::str::contains(
            "warn chat: the line \"- 2026-10-10 08:00 engine: typed by…\" is dated before the one above it",
        )
        .and(predicates::str::contains("all good")),
    );
}

#[test]
fn doctor_reports_a_missing_or_outdated_skill() {
    let e = Env::new();
    e.crew().arg("install-hook").assert().success();
    let skill = e.home.join(".claude/skills/crew/SKILL.md");
    fs::write(&skill, "---\nname: crew\n---\nold text\n").unwrap();
    e.crew()
        .arg("doctor")
        .assert()
        .code(1)
        .stdout(predicates::str::contains("fix  no /crew skill, or an older one"));
    e.crew()
        .arg("install-hook")
        .assert()
        .success()
        .stdout(predicates::str::contains("already installed").and(predicates::str::contains("skill installed")));
    e.crew().arg("doctor").assert().code(0);
}

#[test]
fn hook_is_silent_without_configuration_and_honest_about_a_stale_copy() {
    let e = Env::new();
    e.crew()
        .env("MIGHTY_CREW_CONFIG", e.root.join("none.toml"))
        .current_dir(&e.engine)
        .arg("hook")
        .assert()
        .success()
        .stdout(predicates::str::is_empty());

    e.crew().arg("chat").assert().success();
    fs::rename(&e.bare, e.root.join("away.git")).unwrap();
    e.crew().current_dir(&e.engine).arg("hook").assert().success().stdout(
        predicates::str::contains("shown from the last fetched copy").and(predicates::str::contains("seed line")),
    );
    fs::rename(e.root.join("away.git"), &e.bare).unwrap();

    e.write_config(
        &e.default_config()
            .replace("owns = \"the machine\"", "owns = \"the machine\"\ndispatcher = true"),
    );
    let out = String::from_utf8(
        e.crew()
            .current_dir(&e.ops)
            .arg("hook")
            .assert()
            .success()
            .get_output()
            .stdout
            .clone(),
    )
    .unwrap();
    assert!(
        out.lines()
            .next()
            .unwrap()
            .contains("you are the operations role of crew test (the machine), the dispatcher"),
        "{out}"
    );

    e.write_config(
        &e.default_config()
            .replace("path = \"docs/dev/sessions.md\"", "path = \"docs/none.md\""),
    );
    e.crew()
        .current_dir(&e.ops)
        .arg("hook")
        .assert()
        .success()
        .stdout(predicates::str::contains("(no notes yet)"));
}

#[test]
fn install_hook_replaces_an_old_path_and_refuses_broken_settings() {
    let e = Env::new();
    fs::write(e.settings(), "{\"hooks\":{\"SessionStart\":[{\"matcher\":\"\",\"hooks\":[{\"type\":\"command\",\"command\":\"/elsewhere/crew hook\"}]}]}}").unwrap();
    e.crew()
        .arg("install-hook")
        .assert()
        .success()
        .stdout(predicates::str::contains("hook installed"));
    let s: serde_json::Value = serde_json::from_str(&fs::read_to_string(e.settings()).unwrap()).unwrap();
    let starts = s["hooks"]["SessionStart"].as_array().unwrap();
    assert_eq!(starts.len(), 1);
    let cmd = starts[0]["hooks"][0]["command"].as_str().unwrap();
    assert!(cmd.ends_with("/crew hook") && !cmd.starts_with("/elsewhere"), "{cmd}");

    fs::write(e.settings(), "{ not json").unwrap();
    e.crew()
        .arg("install-hook")
        .assert()
        .failure()
        .stderr(predicates::str::contains("not valid JSON"));
    fs::write(e.settings(), "[]").unwrap();
    e.crew()
        .arg("install-hook")
        .assert()
        .failure()
        .stderr(predicates::str::contains("not a JSON object"));

    // No settings file at all: one is created with just the hook.
    fs::remove_file(e.settings()).unwrap();
    e.crew().arg("install-hook").assert().success();
    let s: serde_json::Value = serde_json::from_str(&fs::read_to_string(e.settings()).unwrap()).unwrap();
    assert_eq!(s["hooks"]["SessionStart"].as_array().unwrap().len(), 1);
    e.crew().arg("remove-hook").assert().success();
    e.crew()
        .arg("remove-hook")
        .assert()
        .success()
        .stdout(predicates::str::contains("no crew hook"));
}

#[test]
fn init_takes_identity_and_default_branch_from_the_repository() {
    let e = Env::new();
    let fresh = e.root.join("fresh.toml");
    git(&e.engine, &["remote", "add", "origin", e.bare.to_str().unwrap()]);
    git(&e.engine, &["fetch", "-q", "origin"]);
    git(&e.engine, &["remote", "set-head", "origin", "main"]);
    git(&e.engine, &["config", "user.name", "Chat Bot"]);
    git(&e.engine, &["config", "user.email", "bot@example.org"]);
    e.crew()
        .env("MIGHTY_CREW_CONFIG", &fresh)
        .current_dir(&e.engine)
        .arg("init")
        .assert()
        .success();
    let text = fs::read_to_string(&fresh).unwrap();
    assert!(
        text.contains("identity = { name = \"Chat Bot\", email = \"bot@example.org\" }"),
        "{text}"
    );
    assert!(
        text.contains("branch = \"main\"")
            && text.contains("[crew]\nname = \"engine\"")
            && text.contains("msg_model = \"haiku\""),
        "{text}"
    );
    // The written file is a valid configuration for every command.
    e.crew()
        .env("MIGHTY_CREW_CONFIG", &fresh)
        .current_dir(&e.engine)
        .arg("status")
        .assert()
        .success()
        .stdout(predicates::str::contains("engine"));
    // Without an origin remote there is no chat to point at.
    e.crew()
        .env("MIGHTY_CREW_CONFIG", e.root.join("other.toml"))
        .current_dir(&e.ops)
        .arg("init")
        .assert()
        .failure()
        .stderr(predicates::str::contains("no `origin` remote"));
}

#[test]
fn usage_errors_exit_2_and_version_prints() {
    let e = Env::new();
    e.crew().assert().code(2);
    e.crew().arg("bogus").assert().code(2);
    e.crew().arg("note").assert().code(2);
    e.crew().args(["msg", "engine"]).assert().code(2);
    e.crew()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicates::str::starts_with("crew 0."));
    e.crew().arg("--help").assert().success().stdout(
        predicates::str::contains("install-hook").and(
            predicates::str::contains("hook")
                .not()
                .or(predicates::str::contains("Hook").not()),
        ),
    );
}
