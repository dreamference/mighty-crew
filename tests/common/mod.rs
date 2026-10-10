//! The offline harness: a bare repository stands in for the chat's remote, a shell script for the
//! agent, and every path the product reads points into the test's own directory.
#![allow(dead_code)]

use assert_cmd::Command;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

pub struct Env {
    _tmp: TempDir,
    pub root: PathBuf,
    pub home: PathBuf,
    pub config: PathBuf,
    pub bare: PathBuf,
    pub stub: PathBuf,
    pub ops: PathBuf,
    pub product: PathBuf,
    pub engine: PathBuf,
}

pub fn git(dir: &Path, args: &[&str]) -> String {
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

pub fn init_repo(dir: &Path) {
    fs::create_dir_all(dir).unwrap();
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.name", "t"]);
    git(dir, &["config", "user.email", "t@t"]);
    fs::write(dir.join("README"), "x\n").unwrap();
    git(dir, &["add", "README"]);
    git(dir, &["commit", "-q", "-m", "init"]);
}

pub const STUB_SCRIPT: &str = "#!/bin/bash\n\
printf '%s\\n' \"$@\" > \"$STUB_LOG\"\n\
pwd -P > \"$STUB_LOG.cwd\"\n\
if [ \"$1\" = agents ]; then\n\
  if [ -n \"$STUB_AGENTS_EXIT\" ]; then echo 'listing broken' >&2; exit \"$STUB_AGENTS_EXIT\"; fi\n\
  if [ -s \"$STUB_AGENTS\" ]; then cat \"$STUB_AGENTS\"; else echo '[]'; fi; exit 0\n\
fi\n\
if [ \"$1\" = -p ]; then\n\
  if [ -n \"$STUB_P_EXIT\" ]; then echo 'relay broken' >&2; exit \"$STUB_P_EXIT\"; fi\n\
  if [ -s \"${STUB_P:-/dev/null}\" ]; then cat \"$STUB_P\"; else echo '{\"result\":\"DELIVERED: sent\"}'; fi; exit 0\n\
fi\n\
echo \"stub agent started\"\n";

pub const CHAT_SEED: &str = "# test: the crew's chat\n\nhow to append\n\n| role | working tree | owns |\n| --- | --- | --- |\n\n## Status\n\n- 2026-10-10 09:00 operations: seed line\n";

impl Env {
    pub fn new() -> Env {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let home = root.join("home");
        fs::create_dir_all(home.join(".config/mighty-crew")).unwrap();
        fs::create_dir_all(home.join(".claude")).unwrap();

        // The chat's remote: a bare repository seeded with one chat file.
        let bare = root.join("chat.git");
        fs::create_dir_all(&bare).unwrap();
        git(&bare, &["init", "-q", "--bare", "-b", "main"]);
        let seed = root.join("seed");
        init_repo(&seed);
        fs::create_dir_all(seed.join("docs/dev")).unwrap();
        fs::write(seed.join("docs/dev/sessions.md"), CHAT_SEED).unwrap();
        git(&seed, &["add", "."]);
        git(&seed, &["commit", "-q", "-m", "chat"]);
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
        fs::write(&stub, STUB_SCRIPT).unwrap();
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

    pub fn default_config(&self) -> String {
        format!(
            "[crew]\nname = \"test\"\n\n[chat]\nrepo = {bare:?}\npath = \"docs/dev/sessions.md\"\nbranch = \"main\"\nidentity = {{ name = \"chat-bot\", email = \"bot@example.org\" }}\n\n\
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

    pub fn write_config(&self, text: &str) {
        fs::write(&self.config, text).unwrap();
    }

    pub fn crew(&self) -> Command {
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

    pub fn stub_args(&self) -> Vec<String> {
        fs::read_to_string(self.root.join("stub.log"))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    pub fn stub_cwd(&self) -> String {
        fs::read_to_string(self.root.join("stub.log.cwd"))
            .unwrap_or_default()
            .trim()
            .to_string()
    }

    pub fn sessions(&self, json: &str) {
        fs::write(self.root.join("agents.json"), json).unwrap();
    }

    pub fn chat_on_remote(&self) -> String {
        git(&self.bare, &["show", "main:docs/dev/sessions.md"])
    }

    pub fn settings(&self) -> PathBuf {
        self.home.join(".claude/settings.json")
    }

    /// The same command as `crew()`, as a plain process for running several at once.
    pub fn crew_std(&self) -> std::process::Command {
        let mut c = std::process::Command::new(assert_cmd::cargo::cargo_bin("crew"));
        c.env_clear();
        for (k, v) in self.envs() {
            c.env(k, v);
        }
        c.current_dir(&self.root);
        c
    }

    pub fn envs(&self) -> Vec<(&'static str, String)> {
        let p = |x: PathBuf| x.to_string_lossy().into_owned();
        vec![
            ("PATH", std::env::var("PATH").unwrap_or_default()),
            ("HOME", p(self.home.clone())),
            ("XDG_CONFIG_HOME", p(self.home.join(".config"))),
            ("XDG_CACHE_HOME", p(self.home.join(".cache"))),
            ("CLAUDE_CONFIG_DIR", p(self.home.join(".claude"))),
            ("MIGHTY_CREW_CONFIG", p(self.config.clone())),
            ("GIT_CONFIG_GLOBAL", "/dev/null".into()),
            ("GIT_CONFIG_SYSTEM", "/dev/null".into()),
            ("GIT_AUTHOR_NAME", "t".into()),
            ("GIT_AUTHOR_EMAIL", "t@t".into()),
            ("GIT_COMMITTER_NAME", "t".into()),
            ("GIT_COMMITTER_EMAIL", "t@t".into()),
            ("STUB_LOG", p(self.root.join("stub.log"))),
            ("STUB_AGENTS", p(self.root.join("agents.json"))),
            ("STUB_P", p(self.root.join("p.json"))),
        ]
    }

    /// The status lines of the chat as the remote holds it.
    pub fn remote_lines(&self) -> Vec<String> {
        let text = self.chat_on_remote();
        let i = text
            .find("## Status")
            .map(|i| i + "## Status".len())
            .unwrap_or(text.len());
        text[i..]
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(str::to_string)
            .collect()
    }

    /// Write a chat file straight onto the remote, as a hand edit would.
    pub fn write_remote_chat(&self, content: &str) {
        let w = self.root.join("hand");
        if !w.exists() {
            git(
                &self.root,
                &["clone", "-q", self.bare.to_str().unwrap(), w.to_str().unwrap()],
            );
            git(&w, &["config", "user.name", "t"]);
            git(&w, &["config", "user.email", "t@t"]);
        } else {
            git(&w, &["pull", "-q", "--rebase", "origin", "main"]);
        }
        fs::create_dir_all(w.join("docs/dev")).unwrap();
        fs::write(w.join("docs/dev/sessions.md"), content).unwrap();
        git(&w, &["add", "."]);
        git(&w, &["commit", "-q", "-m", "hand edit"]);
        git(&w, &["push", "-q", "origin", "main"]);
    }
}

/// A background row of the listing.
pub fn background_json(cwd: &Path, name: &str, state: &str) -> String {
    format!(
        "{{\"id\":\"bg1\",\"cwd\":{:?},\"kind\":\"background\",\"name\":{:?},\"sessionId\":\"bg\",\"state\":{:?}}}",
        cwd.to_str().unwrap(),
        name,
        state
    )
}

pub fn session_json(cwd: &Path, name: &str, status: &str) -> String {
    format!(
        "[{{\"cwd\":{:?},\"kind\":\"interactive\",\"name\":{:?},\"sessionId\":\"abc\",\"status\":{:?},\"pid\":1}}]",
        cwd.to_str().unwrap(),
        name,
        status
    )
}

/// `git` with extra environment, for a backdated commit and the like.
pub fn git_env(dir: &Path, args: &[&str], env: &[(&str, &str)]) -> String {
    let mut c = std::process::Command::new("git");
    c.arg("-C").arg(dir).args(args);
    c.env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null");
    for (k, v) in [
        ("GIT_AUTHOR_NAME", "t"),
        ("GIT_AUTHOR_EMAIL", "t@t"),
        ("GIT_COMMITTER_NAME", "t"),
        ("GIT_COMMITTER_EMAIL", "t@t"),
    ] {
        c.env(k, v);
    }
    for (k, v) in env {
        c.env(k, v);
    }
    let out = c.output().expect("git runs");
    assert!(
        out.status.success(),
        "git {:?}: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim_end().to_string()
}
