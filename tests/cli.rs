//! The offline suite: a bare repository stands in for the board's remote, a shell script for the
//! agent, and every path the product reads points into the test's own directory.

use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

struct Env {
    _tmp: TempDir,
    root: PathBuf,
    home: PathBuf,
    config: PathBuf,
    bare: PathBuf,
    stub: PathBuf,
    ops: PathBuf,
    product: PathBuf,
    engine: PathBuf,
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .output()
        .expect("git runs");
    assert!(
        out.status.success(),
        "git {:?} in {}: {}",
        args,
        dir.display(),
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim_end().to_string()
}

fn init_repo(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.name", "t"]);
    git(dir, &["config", "user.email", "t@t"]);
    fs::write(dir.join("README"), "x\n").unwrap();
    git(dir, &["add", "README"]);
    git(dir, &["commit", "-q", "-m", "init"]);
}

const BOARD_SEED: &str = "# test: the crew's board\n\nhow to append\n\n| role | working tree | owns |\n| --- | --- | --- |\n\n## Status\n\n- 2026-10-10 09:00 operations: seed line\n";

impl Env {
    fn new() -> Env {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let home = root.join("home");
        fs::create_dir_all(home.join(".config/mighty-crew")).unwrap();
        fs::create_dir_all(home.join(".claude")).unwrap();

        // The board's remote: a bare repository seeded with one board file.
        let bare = root.join("board.git");
        fs::create_dir_all(&bare).unwrap();
        git(&bare, &["init", "-q", "--bare", "-b", "main"]);
        let seed = root.join("seed");
        init_repo(&seed);
        fs::create_dir_all(seed.join("docs/dev")).unwrap();
        fs::write(seed.join("docs/dev/sessions.md"), BOARD_SEED).unwrap();
        git(&seed, &["add", "."]);
        git(&seed, &["commit", "-q", "-m", "board"]);
        git(&seed, &["remote", "add", "origin", bare.to_str().unwrap()]);
        git(&seed, &["push", "-q", "origin", "main"]);

        // Three trees: operations, product (a worktree nested inside operations), engine.
        let ops = root.join("ops");
        init_repo(&ops);
        let product = ops.join(".claude/worktrees/product");
        fs::create_dir_all(ops.join(".claude/worktrees")).unwrap();
        git(
            &ops,
            &["worktree", "add", "-q", "-b", "work/product", product.to_str().unwrap()],
        );
        let engine = root.join("engine");
        init_repo(&engine);

        // The agent: a script that records its arguments and answers from files.
        let bin = root.join("bin");
        fs::create_dir_all(&bin).unwrap();
        let stub = bin.join("claude");
        fs::write(
            &stub,
            "#!/bin/bash\n\
             printf '%s\\n' \"$@\" > \"$STUB_LOG\"\n\
             pwd -P > \"$STUB_LOG.cwd\"\n\
             if [ \"$1\" = agents ]; then if [ -s \"$STUB_AGENTS\" ]; then cat \"$STUB_AGENTS\"; else echo '[]'; fi; exit 0; fi\n\
             if [ \"$1\" = -p ]; then cat \"${STUB_P:-/dev/null}\"; [ -s \"${STUB_P:-/dev/null}\" ] || echo '{\"result\":\"DELIVERED: sent\"}'; exit 0; fi\n\
             echo \"stub agent started\"\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&stub, fs::Permissions::from_mode(0o755)).unwrap();

        let env = Env {
            _tmp: tmp,
            root,
            home,
            config: PathBuf::new(),
            bare,
            stub,
            ops,
            product,
            engine,
        };
        let env = Env {
            config: env.home.join(".config/mighty-crew/crew.toml"),
            ..env
        };
        env.write_config(&env.default_config());
        env
    }

    fn default_config(&self) -> String {
        format!(
            "[crew]\nname = \"test\"\n\n[board]\nrepo = {bare:?}\npath = \"docs/dev/sessions.md\"\nbranch = \"main\"\nidentity = {{ name = \"board-bot\", email = \"bot@example.org\" }}\n\n\
             [agent]\ncommand = {stub:?}\npermission_mode = \"bypassPermissions\"\n\n\
             [roles.operations]\ntree = {ops:?}\nowns = \"the machine\"\n\n\
             [roles.product]\ntree = {product:?}\nworktree_of = {ops:?}\nbranch = \"work/product\"\nowns = \"the code\"\n\n\
             [roles.engine]\ntree = {engine:?}\nowns = \"the engine\"\n",
            bare = self.bare.to_str().unwrap(),
            stub = self.stub.to_str().unwrap(),
            ops = self.ops.to_str().unwrap(),
            product = self.product.to_str().unwrap(),
            engine = self.engine.to_str().unwrap(),
        )
    }

    fn write_config(&self, text: &str) {
        fs::write(&self.config, text).unwrap();
    }

    fn crew(&self) -> Command {
        let mut c = Command::cargo_bin("crew").unwrap();
        c.env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", self.home.join(".config"))
            .env("XDG_CACHE_HOME", self.home.join(".cache"))
            .env("CLAUDE_CONFIG_DIR", self.home.join(".claude"))
            .env("MIGHTY_CREW_CONFIG", &self.config)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .env("STUB_LOG", self.root.join("stub.log"))
            .env("STUB_AGENTS", self.root.join("agents.json"))
            .env("STUB_P", self.root.join("p.json"))
            .current_dir(&self.root);
        c
    }

    fn stub_args(&self) -> Vec<String> {
        fs::read_to_string(self.root.join("stub.log"))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn stub_cwd(&self) -> String {
        fs::read_to_string(self.root.join("stub.log.cwd"))
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    fn sessions(&self, json: &str) {
        fs::write(self.root.join("agents.json"), json).unwrap();
    }

    fn board_on_remote(&self) -> String {
        git(&self.bare, &["show", "main:docs/dev/sessions.md"])
    }

    fn settings(&self) -> PathBuf {
        self.home.join(".claude/settings.json")
    }
}

fn session_json(cwd: &Path, name: &str, status: &str) -> String {
    format!(
        "[{{\"cwd\":{:?},\"kind\":\"interactive\",\"name\":{:?},\"sessionId\":\"abc\",\"status\":{:?},\"pid\":1}}]",
        cwd.to_str().unwrap(),
        name,
        status
    )
}

#[test]
fn config_with_two_roles_on_one_tree_is_refused() {
    let e = Env::new();
    let bad = e.default_config().replace(
        &format!("tree = {:?}\nowns = \"the engine\"", e.engine.to_str().unwrap()),
        &format!("tree = {:?}\nowns = \"the engine\"", e.ops.to_str().unwrap()),
    );
    e.write_config(&bad);
    e.crew()
        .arg("status")
        .assert()
        .failure()
        .stderr(predicates::str::contains("share the working tree"));
}

#[test]
fn note_is_written_as_the_role_of_the_current_directory() {
    let e = Env::new();
    // The product tree nests inside the operations tree: the deepest tree wins.
    e.crew()
        .current_dir(&e.product)
        .args(["note", "release", "branch  prepared"])
        .assert()
        .success()
        .stdout(
            predicates::str::is_match(r"^- \d{4}-\d{2}-\d{2} \d{2}:\d{2} product: release branch prepared\n$").unwrap(),
        );
    let board = e.board_on_remote();
    assert!(board.ends_with("product: release branch prepared"), "{board}");
    assert_eq!(
        git(&e.bare, &["log", "-1", "--format=%s %an <%ae>", "main"]),
        "board: product board-bot <bot@example.org>"
    );
    // The role's own tree was not touched.
    assert_eq!(git(&e.product, &["status", "--porcelain"]), "");
    // And the note is readable from any role.
    e.crew()
        .current_dir(&e.engine)
        .args(["board", "-n", "1"])
        .assert()
        .success()
        .stdout(predicates::str::ends_with("product: release branch prepared\n"));
    e.crew()
        .args(["board", "--all"])
        .assert()
        .success()
        .stdout(predicates::str::contains("operations: seed line"));
    e.crew()
        .args(["board", "--for", "product"])
        .assert()
        .success()
        .stdout(predicates::str::contains("product: release").and(predicates::str::contains("seed line").not()));
}

#[test]
fn note_outside_any_tree_needs_as() {
    let e = Env::new();
    e.crew()
        .args(["note", "hello"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("in no role's tree"));
    e.crew()
        .args(["note", "--as", "engine", "hello"])
        .assert()
        .success()
        .stdout(predicates::str::contains(" engine: hello"));
}

#[test]
fn two_roles_with_stale_clones_both_land_their_notes() {
    let e = Env::new();
    // Two caches: the second clone is made before the first note lands, so its push is rejected once.
    let other_cache = e.root.join("cache2");
    e.crew()
        .env("XDG_CACHE_HOME", &other_cache)
        .args(["board"])
        .assert()
        .success();
    e.crew().args(["note", "--as", "engine", "first"]).assert().success();
    e.crew()
        .env("XDG_CACHE_HOME", &other_cache)
        .args(["note", "--as", "product", "second"])
        .assert()
        .success();
    let board = e.board_on_remote();
    assert!(
        board.contains(" engine: first\n") && board.ends_with(" product: second"),
        "{board}"
    );
}

#[test]
fn first_note_on_an_empty_remote_creates_the_board() {
    let e = Env::new();
    let empty = e.root.join("empty.git");
    fs::create_dir_all(&empty).unwrap();
    git(&empty, &["init", "-q", "--bare", "-b", "main"]);
    e.write_config(
        &e.default_config()
            .replace(e.bare.to_str().unwrap(), empty.to_str().unwrap()),
    );
    e.crew()
        .args(["board"])
        .assert()
        .success()
        .stdout(predicates::str::contains("does not exist yet"));
    e.crew().args(["note", "--as", "engine", "born"]).assert().success();
    let board = git(&empty, &["show", "main:docs/dev/sessions.md"]);
    assert!(board.starts_with("# test: the crew's board"), "{board}");
    assert!(
        board.contains("| engine | `") && board.contains("## Status") && board.ends_with(" engine: born"),
        "{board}"
    );
}

#[test]
fn start_runs_the_agent_in_the_tree_with_name_and_mode() {
    let e = Env::new();
    e.crew().args(["start", "engine"]).assert().success().stdout(
        predicates::str::contains("engine role of crew test")
            .and(predicates::str::contains("seed line"))
            .and(predicates::str::contains("starting a fresh one"))
            .and(predicates::str::contains("stub agent started")),
    );
    assert_eq!(
        e.stub_args(),
        vec!["--name", "engine", "--permission-mode", "bypassPermissions"]
    );
    assert_eq!(e.stub_cwd(), e.engine.to_str().unwrap());

    // With a conversation in the directory, --continue.
    let mangled: String = e
        .engine
        .to_str()
        .unwrap()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let project = e.home.join(".claude/projects").join(mangled);
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("s.jsonl"), "{}\n").unwrap();
    e.crew().args(["start", "engine"]).assert().success();
    assert_eq!(
        e.stub_args(),
        vec![
            "--name",
            "engine",
            "--permission-mode",
            "bypassPermissions",
            "--continue"
        ]
    );
    e.crew().args(["start", "engine", "--new"]).assert().success();
    assert_eq!(
        e.stub_args(),
        vec!["--name", "engine", "--permission-mode", "bypassPermissions"]
    );
}

#[test]
fn start_refuses_a_tree_with_a_live_session_and_a_missing_tree() {
    let e = Env::new();
    e.sessions(&session_json(&e.engine, "engine", "busy"));
    e.crew()
        .args(["start", "engine"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("already running"));
    e.crew()
        .args(["start", "nobody"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no role named nobody"));

    // A missing worktree is created on request, from the configured repository and branch.
    let e = Env::new();
    let cfg = e
        .default_config()
        .replace("branch = \"work/product\"", "branch = \"work/other\"")
        .replace(e.product.to_str().unwrap(), e.ops.join("wt/other").to_str().unwrap());
    e.write_config(&cfg);
    e.crew()
        .args(["start", "product"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("is missing").and(predicates::str::contains("--create")));
    e.crew()
        .args(["start", "product", "--create"])
        .assert()
        .success()
        .stdout(predicates::str::contains("created the worktree"));
    assert_eq!(
        git(&e.ops.join("wt/other"), &["branch", "--show-current"]),
        "work/other"
    );
    assert_eq!(
        e.stub_cwd(),
        e.ops.join("wt/other").canonicalize().unwrap().to_str().unwrap()
    );
}

#[test]
fn hook_install_remove_and_session_start() {
    let e = Env::new();
    fs::write(e.settings(), "{\n  \"theme\": \"dark\",\n  \"hooks\": {\"SessionStart\": [{\"matcher\": \"\", \"hooks\": [{\"type\": \"command\", \"command\": \"/usr/bin/other-hook\"}]}]}\n}\n").unwrap();
    e.crew()
        .arg("install-hook")
        .assert()
        .success()
        .stdout(predicates::str::contains("hook installed"));
    let s: serde_json::Value = serde_json::from_str(&fs::read_to_string(e.settings()).unwrap()).unwrap();
    assert_eq!(s["theme"], "dark");
    let starts = s["hooks"]["SessionStart"].as_array().unwrap();
    assert_eq!(starts.len(), 2);
    assert_eq!(starts[0]["hooks"][0]["command"], "/usr/bin/other-hook");
    let cmd = starts[1]["hooks"][0]["command"].as_str().unwrap().to_string();
    assert!(cmd.ends_with("/crew hook"), "{cmd}");
    e.crew()
        .arg("install-hook")
        .assert()
        .success()
        .stdout(predicates::str::contains("already installed"));
    let skill = e.home.join(".claude/skills/crew/SKILL.md");
    let text = fs::read_to_string(&skill).unwrap();
    assert!(
        text.starts_with("---\nname: crew\n") && text.contains("crew $ARGUMENTS"),
        "{text}"
    );

    // The hook prints the board in a role's tree and nothing elsewhere.
    e.crew()
        .current_dir(&e.product)
        .arg("hook")
        .assert()
        .success()
        .stdout(predicates::str::contains("you are the product role").and(predicates::str::contains("seed line")));
    e.crew()
        .current_dir(&e.root)
        .arg("hook")
        .assert()
        .success()
        .stdout(predicates::str::is_empty());

    e.crew()
        .arg("remove-hook")
        .assert()
        .success()
        .stdout(predicates::str::contains("hook removed"));
    assert!(!skill.exists());
    let s: serde_json::Value = serde_json::from_str(&fs::read_to_string(e.settings()).unwrap()).unwrap();
    assert_eq!(s["hooks"]["SessionStart"].as_array().unwrap().len(), 1);
    assert_eq!(s["theme"], "dark");
}

#[test]
fn doctor_reports_each_problem_with_its_fix() {
    let e = Env::new();
    e.crew().arg("install-hook").assert().success();
    e.crew()
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicates::str::contains("all good"));

    // A missing tree, an off-class role, a wrong hook path, a stray session, a missing board file.
    let cfg = e
        .default_config()
        .replace(e.engine.to_str().unwrap(), e.root.join("gone").to_str().unwrap())
        .replace(
            "owns = \"the code\"",
            "owns = \"the code\"\npermission_mode = \"acceptEdits\"",
        )
        .replace("path = \"docs/dev/sessions.md\"", "path = \"docs/other.md\"");
    e.write_config(&cfg);
    fs::write(e.settings(), "{\"hooks\":{\"SessionStart\":[{\"matcher\":\"\",\"hooks\":[{\"type\":\"command\",\"command\":\"/elsewhere/crew hook\"}]}]}}").unwrap();
    e.sessions(&session_json(&e.root, "stray-1a", "idle"));
    let out = e.crew().arg("doctor").assert().code(1).get_output().stdout.clone();
    let out = String::from_utf8(out).unwrap();
    for needle in [
        "role engine: tree",
        "is missing",
        "role product runs in permission mode acceptEdits while the crew runs in bypassPermissions",
        "board file docs/other.md is missing",
        "runs `/elsewhere/crew hook`, not this binary",
        "which is no role's tree",
        "no /crew skill",
    ] {
        assert!(out.contains(needle), "missing {needle:?} in:\n{out}");
    }
}

#[test]
fn doctor_warns_about_a_session_started_in_another_class() {
    let e = Env::new();
    e.crew().arg("install-hook").assert().success();
    e.crew().args(["start", "engine"]).assert().success();
    e.sessions(&session_json(&e.engine, "engine", "busy"));
    e.crew()
        .arg("doctor")
        .assert()
        .success()
        .stdout(predicates::str::contains(
            "session engine (busy) started in bypassPermissions",
        ));
    e.write_config(&e.default_config().replace(
        "permission_mode = \"bypassPermissions\"",
        "permission_mode = \"acceptEdits\"",
    ));
    e.crew().arg("doctor").assert().code(1).stdout(
        predicates::str::contains("started in bypassPermissions, the configuration now says acceptEdits")
            .and(predicates::str::contains("restart it")),
    );
}

#[test]
fn status_lists_every_role() {
    let e = Env::new();
    e.sessions(&session_json(&e.product, "product", "busy"));
    e.crew()
        .current_dir(&e.engine)
        .args(["note", "engine", "measured"])
        .assert()
        .success();
    let out = String::from_utf8(e.crew().arg("status").assert().success().get_output().stdout.clone()).unwrap();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 4, "{out}");
    assert!(
        lines[1].starts_with("operations")
            && lines[1].contains("main")
            && lines[1].contains("not running")
            && lines[1].contains("09:00 seed line"),
        "{out}"
    );
    assert!(
        lines[2].starts_with("product") && lines[2].contains("work/product") && lines[2].contains("busy (product)"),
        "{out}"
    );
    assert!(
        lines[3].starts_with("engine") && lines[3].contains("engine measured"),
        "{out}"
    );
}

#[test]
fn msg_writes_the_board_twin_and_relays_through_the_agent() {
    let e = Env::new();
    e.crew()
        .args(["msg", "product", "hello"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("no session is running"));
    e.sessions(&session_json(&e.product, "product", "idle"));
    e.crew()
        .current_dir(&e.engine)
        .args(["msg", "product", "the API moved;", "section 7 is yours"])
        .assert()
        .success()
        .stdout(
            predicates::str::contains(" engine: to product: the API moved; section 7 is yours")
                .and(predicates::str::contains("in the inbox of product (product)")),
        );
    let args = e.stub_args();
    assert_eq!(
        &args[..7],
        [
            "-p",
            "--permission-mode",
            "bypassPermissions",
            "--name",
            "crew-msg",
            "--model",
            "haiku"
        ]
    );
    let joined = args.join("\n");
    assert!(
        args.contains(&"SendMessage".to_string())
            && joined.contains("to=\"product\"")
            && joined.ends_with("Message:\nthe API moved; section 7 is yours"),
        "{args:?}"
    );
    assert!(e
        .board_on_remote()
        .ends_with(" engine: to product: the API moved; section 7 is yours"));

    fs::write(e.root.join("p.json"), "{\"result\":\"HELD: held for approval\"}").unwrap();
    e.crew()
        .current_dir(&e.engine)
        .args(["msg", "product", "again"])
        .assert()
        .code(1)
        .stdout(predicates::str::contains("held by product"));
    e.crew()
        .current_dir(&e.product)
        .args(["msg", "product", "to myself"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("the role you are in"));
}

#[test]
fn init_writes_a_configuration_from_the_repository() {
    let e = Env::new();
    let fresh = e.root.join("fresh.toml");
    git(&e.engine, &["remote", "add", "origin", e.bare.to_str().unwrap()]);
    e.crew()
        .env("MIGHTY_CREW_CONFIG", &fresh)
        .current_dir(&e.engine)
        .arg("init")
        .assert()
        .success()
        .stdout(predicates::str::contains("wrote"));
    let text = fs::read_to_string(&fresh).unwrap();
    assert!(
        text.contains(&format!("repo = {:?}", e.bare.to_str().unwrap())) && text.contains("[roles.engine]"),
        "{text}"
    );
    e.crew()
        .env("MIGHTY_CREW_CONFIG", &fresh)
        .current_dir(&e.engine)
        .arg("init")
        .assert()
        .failure()
        .stderr(predicates::str::contains("exists"));
    e.crew()
        .env("MIGHTY_CREW_CONFIG", &fresh)
        .current_dir(&e.engine)
        .args(["init", "--force"])
        .assert()
        .success();
    e.crew()
        .env("MIGHTY_CREW_CONFIG", &fresh)
        .current_dir(&e.root)
        .args(["init", "--force"])
        .assert()
        .failure()
        .stderr(predicates::str::contains("not inside a git repository"));
}

#[test]
fn open_without_a_display_prints_the_command() {
    let e = Env::new();
    e.crew()
        .args(["open", "engine"])
        .assert()
        .success()
        .stdout(predicates::str::contains("no display").and(predicates::str::contains("start engine")));
}
