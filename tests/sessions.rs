//! start, status, msg and open against the stub agent and real git trees.
#![cfg(unix)]
mod common;
use common::*;
#[allow(unused_imports)]
use predicates::prelude::*;
use std::fs;
use std::path::Path;

/// An upstream for a tree: a bare repository the tree pushes to, and a second clone to commit from.
fn upstream(e: &Env, tree: &Path, name: &str) -> std::path::PathBuf {
    let bare = e.root.join(format!("{name}.git"));
    fs::create_dir_all(&bare).unwrap();
    git(&bare, &["init", "-q", "--bare", "-b", "main"]);
    git(tree, &["remote", "add", "origin", bare.to_str().unwrap()]);
    git(tree, &["push", "-q", "-u", "origin", "HEAD"]);
    let other = e.root.join(format!("{name}-other"));
    git(
        &e.root,
        &["clone", "-q", bare.to_str().unwrap(), other.to_str().unwrap()],
    );
    git(&other, &["config", "user.name", "t"]);
    git(&other, &["config", "user.email", "t@t"]);
    other
}

fn commit_in(dir: &Path, subject: &str) {
    fs::write(dir.join(subject), "x\n").unwrap();
    git(dir, &["add", "."]);
    git(dir, &["commit", "-q", "-m", subject]);
    git(dir, &["push", "-q", "origin", "HEAD"]);
}

#[test]
fn start_fetches_and_fast_forwards_a_clean_plain_checkout_only() {
    let e = Env::new();
    let other = upstream(&e, &e.engine, "engine-up");
    commit_in(&other, "new-upstream");
    e.crew().args(["start", "engine"]).assert().success();
    assert_eq!(git(&e.engine, &["log", "-1", "--format=%s"]), "new-upstream");

    // A dirty tree is left alone.
    fs::write(e.engine.join("scratch"), "wip\n").unwrap();
    commit_in(&other, "newer");
    e.crew().args(["start", "engine"]).assert().success();
    assert_eq!(git(&e.engine, &["log", "-1", "--format=%s"]), "new-upstream");
    assert_eq!(git(&e.engine, &["status", "--porcelain"]), "?? scratch");
}

#[test]
fn start_does_not_fast_forward_a_worktree() {
    let e = Env::new();
    // The product worktree's branch gets a commit upstream; the worktree keeps its own head.
    let other = upstream(&e, &e.product, "ops-up");
    git(&other, &["checkout", "-q", "work/product"]);
    commit_in(&other, "upstream-product");
    let before = git(&e.product, &["rev-parse", "HEAD"]);
    e.crew().args(["start", "product"]).assert().success();
    assert_eq!(git(&e.product, &["rev-parse", "HEAD"]), before);
    assert_eq!(
        git(&e.product, &["log", "-1", "--format=%s", "origin/work/product"]),
        "upstream-product"
    );
}

#[test]
fn start_refuses_a_directory_that_is_not_a_checkout() {
    let e = Env::new();
    let plain = e.root.join("plain");
    fs::create_dir_all(&plain).unwrap();
    e.write_config(
        &e.default_config()
            .replace(e.engine.to_str().unwrap(), plain.to_str().unwrap()),
    );
    e.crew()
        .args(["start", "engine"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("is not a git checkout"));
}

#[test]
fn start_goes_on_when_the_listing_fails_and_records_the_mode() {
    let e = Env::new();
    e.crew()
        .env("STUB_AGENTS_EXIT", "3")
        .args(["start", "engine"])
        .assert()
        .success();
    assert_eq!(e.stub_args()[..2], ["--name", "engine"]);
    let record = fs::read_to_string(e.home.join(".cache/mighty-crew/started/engine")).unwrap();
    assert!(
        record.contains(&format!("tree = {:?}", e.engine.to_str().unwrap()))
            && record.contains("permission_mode = \"bypassPermissions\""),
        "{record}"
    );
}

#[test]
fn start_create_reuses_an_existing_branch() {
    let e = Env::new();
    git(&e.ops, &["branch", "work/other"]);
    let tree = e.ops.join("wt/other");
    e.write_config(
        &e.default_config()
            .replace("branch = \"work/product\"", "branch = \"work/other\"")
            .replace(e.product.to_str().unwrap(), tree.to_str().unwrap()),
    );
    e.crew()
        .args(["start", "product", "--create"])
        .assert()
        .success()
        .stdout(predicates::str::contains("created the worktree"));
    assert_eq!(git(&tree, &["branch", "--show-current"]), "work/other");
    assert_eq!(
        git(&tree, &["rev-parse", "HEAD"]),
        git(&e.ops, &["rev-parse", "work/other"])
    );
    // Without worktree_of there is nothing to create from.
    e.write_config(
        &e.default_config()
            .replace(e.engine.to_str().unwrap(), e.root.join("nowhere").to_str().unwrap()),
    );
    e.crew()
        .args(["start", "engine", "--create"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("does not exist"));
}

#[test]
fn status_shows_missing_detached_and_background_sessions() {
    let e = Env::new();
    git(&e.ops, &["checkout", "-q", "--detach"]);
    e.write_config(
        &e.default_config()
            .replace(e.engine.to_str().unwrap(), e.root.join("gone").to_str().unwrap()),
    );
    e.sessions(&format!(
        "[{},{}]",
        background_json(&e.product, "bg-product", "working"),
        background_json(&e.ops, "old", "done")
    ));
    let out = String::from_utf8(e.crew().arg("status").assert().success().get_output().stdout.clone()).unwrap();
    let row = |r: &str| out.lines().find(|l| l.starts_with(r)).unwrap().to_string();
    assert!(
        row("operations").contains("(detached)") && row("operations").contains("not running"),
        "{out}"
    );
    assert!(row("product").contains("working (bg-product)"), "{out}");
    assert!(row("engine").contains("(missing)"), "{out}");
}

#[test]
fn status_survives_a_broken_listing() {
    let e = Env::new();
    let out = e.crew().env("STUB_AGENTS_EXIT", "1").arg("status").assert().success();
    let text = String::from_utf8(out.get_output().stdout.clone()).unwrap();
    assert!(text.lines().count() == 4 && text.contains("not running"), "{text}");
}

#[test]
fn msg_reports_refusals_and_relay_failures_with_the_twin_on_the_chat() {
    let e = Env::new();
    e.sessions(&session_json(&e.product, "product", "idle"));
    fs::write(e.root.join("p.json"), "{\"result\":\"REFUSED: dropped at the inbox\"}").unwrap();
    e.crew()
        .current_dir(&e.engine)
        .args(["msg", "product", "one"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("refused the message"));
    assert!(e.chat_on_remote().ends_with(" engine: to product: one"));

    fs::write(e.root.join("p.json"), "{\"result\":\"I sent it, I think\"}").unwrap();
    e.crew()
        .current_dir(&e.engine)
        .args(["msg", "product", "two"])
        .assert()
        .code(1)
        .stderr(predicates::str::contains("refused"));

    fs::write(e.root.join("p.json"), "not json").unwrap();
    e.crew()
        .current_dir(&e.engine)
        .args(["msg", "product", "three"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("unreadable JSON"));

    e.crew()
        .env("STUB_P_EXIT", "2")
        .current_dir(&e.engine)
        .args(["msg", "product", "four"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("-p failed").and(predicates::str::contains("relay broken")));

    let long = "x".repeat(301);
    e.crew()
        .current_dir(&e.engine)
        .args(["msg", "product", &long])
        .assert()
        .failure()
        .stderr(predicates::str::contains("301 characters"));
    e.crew()
        .current_dir(&e.engine)
        .args(["msg", "product", "   "])
        .assert()
        .failure()
        .stderr(predicates::str::contains("the message is empty"));
    // Every attempt that got past the checks left its twin; the refused ones too.
    let lines = e.remote_lines();
    assert_eq!(
        lines.iter().filter(|l| l.contains("to product:")).count(),
        4,
        "{lines:?}"
    );
}

#[test]
fn msg_from_outside_a_tree_is_signed_user_and_reaches_a_background_session() {
    let e = Env::new();
    e.sessions(&format!("[{}]", background_json(&e.product, "bg-product", "working")));
    e.crew()
        .args(["msg", "product", "hello there"])
        .assert()
        .success()
        .stdout(predicates::str::contains(" user: to product: hello there"));
    let args = e.stub_args();
    assert!(args.join("\n").contains("to=\"bg-product\""), "{args:?}");
    assert!(args.contains(&"--allowedTools".to_string()) && args.contains(&"--no-session-persistence".to_string()));
}

#[test]
fn msg_fails_closed_when_the_chat_is_unreachable() {
    let e = Env::new();
    e.sessions(&session_json(&e.product, "product", "idle"));
    e.crew().arg("chat").assert().success();
    fs::rename(&e.bare, e.root.join("away.git")).unwrap();
    e.crew()
        .current_dir(&e.engine)
        .args(["msg", "product", "lost"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("nothing was sent"));
    assert!(
        e.stub_args().is_empty() || e.stub_args()[0] == "agents",
        "the relay must not have run: {:?}",
        e.stub_args()
    );
}

fn term_stub(dir: &Path, name: &str) -> std::path::PathBuf {
    fs::create_dir_all(dir).unwrap();
    let p = dir.join(name);
    fs::write(&p, "#!/bin/bash\nprintf '%s\\n' \"$@\" > \"$STUB_TERM_LOG\"\n").unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
    p
}

fn wait_for(path: &Path) -> String {
    for _ in 0..100 {
        if let Ok(t) = fs::read_to_string(path) {
            if !t.is_empty() {
                return t;
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
    panic!("{} never written", path.display());
}

#[test]
fn open_runs_the_configured_terminal_with_the_start_command() {
    let e = Env::new();
    let term = term_stub(&e.root.join("terms"), "myterm");
    let log = e.root.join("term.log");
    e.write_config(&(e.default_config() + &format!("\n[terminal]\ncommands = [{:?}]\n", term.to_str().unwrap())));
    e.crew()
        .env("DISPLAY", ":0")
        .env("STUB_TERM_LOG", &log)
        .args(["open", "engine"])
        .assert()
        .success()
        .stdout(predicates::str::contains("opened engine in"));
    let args: Vec<String> = wait_for(&log).lines().map(str::to_string).collect();
    assert_eq!(args[0], "-e");
    assert!(args[1].ends_with("/crew"), "{args:?}");
    assert_eq!(&args[2..], ["start", "engine"]);

    // gnome-terminal takes `--` instead of `-e`.
    let gnome = term_stub(&e.root.join("terms2"), "gnome-terminal");
    fs::remove_file(&log).unwrap();
    e.write_config(&(e.default_config() + &format!("\n[terminal]\ncommands = [{:?}]\n", gnome.to_str().unwrap())));
    e.crew()
        .env("WAYLAND_DISPLAY", "wayland-0")
        .env("STUB_TERM_LOG", &log)
        .args(["open", "engine"])
        .assert()
        .success();
    let args: Vec<String> = wait_for(&log).lines().map(str::to_string).collect();
    assert_eq!(args[0], "--");

    // No terminal at all.
    e.write_config(&(e.default_config() + "\n[terminal]\ncommands = [\"/nonexistent/term\"]\n"));
    e.crew()
        .env("DISPLAY", ":0")
        .args(["open", "engine"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no terminal emulator found"));
    e.crew()
        .env("DISPLAY", ":0")
        .args(["open", "nobody"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no role named nobody"));
}
