//! The configuration of §6: strict parsing, paths, nesting.
#![cfg(unix)]
mod common;
use common::*;
#[allow(unused_imports)]
use predicates::prelude::*;
use std::fs;

#[test]
fn a_missing_configuration_names_the_fix() {
    let e = Env::new();
    e.crew()
        .env("MIGHTY_CREW_CONFIG", e.root.join("none.toml"))
        .arg("status")
        .assert()
        .failure()
        .stderr(predicates::str::contains("no configuration at").and(predicates::str::contains("crew init")));
}

#[test]
fn xdg_config_home_is_used_without_the_override() {
    let e = Env::new();
    // The default path under XDG_CONFIG_HOME is where the harness wrote the file.
    e.crew()
        .env_remove("MIGHTY_CREW_CONFIG")
        .arg("board")
        .assert()
        .success()
        .stdout(predicates::str::contains("seed line"));
    // HOME alone, with XDG_CONFIG_HOME unset, resolves to ~/.config as well.
    e.crew()
        .env_remove("MIGHTY_CREW_CONFIG")
        .env_remove("XDG_CONFIG_HOME")
        .arg("board")
        .assert()
        .success()
        .stdout(predicates::str::contains("seed line"));
}

#[test]
fn strict_parsing_refuses_each_mistake_with_its_place() {
    let e = Env::new();
    let base = e.default_config();
    let cases: Vec<(String, &str)> = vec![
        (
            base.replace(
                "[crew]\nname = \"test\"\n",
                "[crew]\nname = \"test\"\ncolour = \"red\"\n",
            ),
            "not a valid configuration",
        ),
        (
            base.replace(&format!("repo = {:?}\n", e.bare.to_str().unwrap()), ""),
            "not a valid configuration",
        ),
        (base.replace("[roles.engine]", "[roles.Engine]"), "must be lowercase"),
        (base[..base.find("[roles.operations]").unwrap()].to_string(), "no roles"),
        (
            base.replace("owns = \"the machine\"", "owns = \"the machine\"\ndispatcher = true")
                .replace("owns = \"the engine\"", "owns = \"the engine\"\ndispatcher = true"),
            "at most one role is the dispatcher",
        ),
        (
            base.replace("owns = \"the engine\"", "owns = \"the engine\"\nspeed = 3"),
            "[roles.engine] is not valid",
        ),
    ];
    for (text, needle) in cases {
        e.write_config(&text);
        e.crew()
            .arg("status")
            .assert()
            .failure()
            .stderr(predicates::str::contains(needle));
    }
}

#[test]
fn tilde_paths_resolve_against_home() {
    let e = Env::new();
    init_repo(&e.home.join("eng"));
    e.write_config(&e.default_config().replace(
        &format!("tree = {:?}\nowns = \"the engine\"", e.engine.to_str().unwrap()),
        "tree = \"~/eng\"\nowns = \"the engine\"",
    ));
    let out = e.crew().arg("status").assert().success().get_output().stdout.clone();
    let out = String::from_utf8(out).unwrap();
    let line = out.lines().find(|l| l.starts_with("engine")).unwrap();
    assert!(
        line.contains("~/eng") && line.contains(" main ") && !line.contains("(missing)"),
        "{line}"
    );
    e.crew()
        .current_dir(e.home.join("eng"))
        .args(["note", "from the tilde tree"])
        .assert()
        .success()
        .stdout(predicates::str::contains(" engine: from the tilde tree"));
}

#[test]
fn a_symbolic_link_counts_as_the_tree_it_points_to() {
    let e = Env::new();
    let link = e.root.join("engine-link");
    std::os::unix::fs::symlink(&e.engine, &link).unwrap();
    let cfg = e.default_config() + &format!("\n[roles.mirror]\ntree = {:?}\n", link.to_str().unwrap());
    e.write_config(&cfg);
    e.crew()
        .arg("status")
        .assert()
        .failure()
        .stderr(predicates::str::contains(
            "roles engine and mirror share the working tree",
        ));
}

#[test]
fn a_role_may_set_its_own_command_and_mode() {
    let e = Env::new();
    let other = e.root.join("bin/other-agent");
    fs::copy(&e.stub, &other).unwrap();
    e.write_config(&e.default_config().replace(
        "owns = \"the engine\"",
        &format!(
            "owns = \"the engine\"\ncommand = {:?}\npermission_mode = \"acceptEdits\"",
            other.to_str().unwrap()
        ),
    ));
    e.crew().args(["start", "engine"]).assert().success();
    assert_eq!(
        e.stub_args(),
        vec!["--name", "engine", "--permission-mode", "acceptEdits"]
    );
    e.crew()
        .arg("doctor")
        .assert()
        .code(1)
        .stdout(predicates::str::contains(
            "role engine runs in permission mode acceptEdits while the crew runs in bypassPermissions",
        ));
}
