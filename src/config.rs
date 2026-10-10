//! The crew's configuration: `~/.config/mighty-crew/crew.toml`.

use crate::util;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Identity {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrewSection {
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoardSection {
    pub repo: String,
    #[serde(default = "default_chat_path")]
    pub path: String,
    #[serde(default = "default_branch")]
    pub branch: String,
    pub identity: Option<Identity>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSection {
    #[serde(default = "default_command")]
    pub command: String,
    #[serde(default = "default_mode")]
    pub permission_mode: String,
    /// The model of the short headless session `crew msg` relays through; one SendMessage call needs little.
    #[serde(default = "default_msg_model")]
    pub msg_model: String,
}

impl Default for AgentSection {
    fn default() -> Self {
        AgentSection {
            command: default_command(),
            permission_mode: default_mode(),
            msg_model: default_msg_model(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TerminalSection {
    #[serde(default)]
    pub commands: Vec<String>,
}

fn default_chat_path() -> String {
    "docs/dev/chat.md".into()
}
fn default_branch() -> String {
    "main".into()
}
fn default_command() -> String {
    "claude".into()
}
fn default_mode() -> String {
    "bypassPermissions".into()
}
fn default_msg_model() -> String {
    "haiku".into()
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRole {
    tree: String,
    worktree_of: Option<String>,
    branch: Option<String>,
    #[serde(default)]
    owns: String,
    command: Option<String>,
    permission_mode: Option<String>,
    #[serde(default)]
    dispatcher: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Raw {
    crew: CrewSection,
    chat: BoardSection,
    #[serde(default)]
    agent: AgentSection,
    #[serde(default)]
    terminal: TerminalSection,
    #[serde(default)]
    roles: toml::Table,
}

/// One role: a name, a charter, a working tree, the agent command and permission mode it runs with.
#[derive(Debug, Clone)]
pub struct Role {
    pub name: String,
    /// The tree as configured (with `~`), for printing.
    pub tree_text: String,
    /// The tree expanded and normalized, for comparing.
    pub tree: PathBuf,
    pub worktree_of: Option<PathBuf>,
    pub branch: Option<String>,
    pub owns: String,
    pub command: String,
    pub permission_mode: String,
    /// The one role, if any, that relays the user across roles; it assigns nothing and grants nothing.
    pub dispatcher: bool,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub path: PathBuf,
    pub crew: CrewSection,
    pub chat: BoardSection,
    pub agent: AgentSection,
    pub terminal: TerminalSection,
    pub roles: Vec<Role>,
}

impl Config {
    pub fn load() -> Result<Config> {
        let path = util::config_path()?;
        Self::load_from(&path)
    }

    pub fn load_from(path: &Path) -> Result<Config> {
        let text = std::fs::read_to_string(path).with_context(|| {
            format!(
                "no configuration at {}: run `crew init` in the repository, or point MIGHTY_CREW_CONFIG at one",
                util::display_path(path)
            )
        })?;
        Self::parse(&text, path)
    }

    pub fn parse(text: &str, path: &Path) -> Result<Config> {
        let raw: Raw =
            toml::from_str(text).with_context(|| format!("{}: not a valid configuration", util::display_path(path)))?;
        let mut roles = Vec::new();
        for (name, value) in raw.roles {
            if name.is_empty()
                || !name
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
            {
                bail!(
                    "{}: role name {name:?} must be lowercase letters, digits, '-' or '_'",
                    util::display_path(path)
                );
            }
            let r: RawRole = value
                .try_into()
                .with_context(|| format!("{}: [roles.{name}] is not valid", util::display_path(path)))?;
            let tree = util::normalize(&util::expand_tilde(&r.tree)?);
            let worktree_of = match r.worktree_of {
                Some(w) => Some(util::normalize(&util::expand_tilde(&w)?)),
                None => None,
            };
            roles.push(Role {
                name,
                tree_text: r.tree,
                tree,
                worktree_of,
                branch: r.branch,
                owns: r.owns,
                command: r.command.unwrap_or_else(|| raw.agent.command.clone()),
                permission_mode: r.permission_mode.unwrap_or_else(|| raw.agent.permission_mode.clone()),
                dispatcher: r.dispatcher,
            });
        }
        if roles.is_empty() {
            bail!(
                "{}: no roles; add a [roles.<name>] table with a tree",
                util::display_path(path)
            );
        }
        let dispatchers: Vec<&str> = roles.iter().filter(|r| r.dispatcher).map(|r| r.name.as_str()).collect();
        if dispatchers.len() > 1 {
            bail!(
                "{}: at most one role is the dispatcher, not {}",
                util::display_path(path),
                dispatchers.join(" and ")
            );
        }
        for i in 0..roles.len() {
            for j in i + 1..roles.len() {
                if roles[i].tree == roles[j].tree {
                    bail!(
                        "roles {} and {} share the working tree {}; give each role a tree of its own",
                        roles[i].name,
                        roles[j].name,
                        roles[i].tree_text
                    );
                }
            }
        }
        Ok(Config {
            path: path.to_path_buf(),
            crew: raw.crew,
            chat: raw.chat,
            agent: raw.agent,
            terminal: raw.terminal,
            roles,
        })
    }

    pub fn role(&self, name: &str) -> Result<&Role> {
        self.roles
            .iter()
            .find(|r| r.name == name)
            .ok_or_else(|| anyhow::anyhow!("no role named {name}; the roles are: {}", self.role_names().join(", ")))
    }

    /// The role names, owned, in configuration order (the display colours follow this order).
    pub fn role_name_list(&self) -> Vec<String> {
        self.roles.iter().map(|r| r.name.clone()).collect()
    }

    pub fn role_names(&self) -> Vec<&str> {
        self.roles.iter().map(|r| r.name.as_str()).collect()
    }

    /// The role whose tree contains `dir`; the deepest tree wins when trees nest.
    pub fn role_for(&self, dir: &Path) -> Option<&Role> {
        let dir = util::normalize(dir);
        self.roles
            .iter()
            .filter(|r| dir.starts_with(&r.tree))
            .max_by_key(|r| r.tree.as_os_str().len())
    }

    /// The roles whose permission mode differs from the crew's: the crew is meant to run in one class.
    pub fn off_class_roles(&self) -> Vec<&Role> {
        self.roles
            .iter()
            .filter(|r| r.permission_mode != self.agent.permission_mode)
            .collect()
    }
}

/// A configuration for `crew init`: one role, the current repository as the chat's.
pub fn render_initial(
    crew_name: &str,
    repo_url: &str,
    branch: &str,
    identity: Option<&Identity>,
    role_name: &str,
    tree_text: &str,
) -> String {
    let identity = match identity {
        Some(i) => format!("identity = {{ name = {:?}, email = {:?} }}\n", i.name, i.email),
        None => "# identity = { name = \"crew\", email = \"crew@example.org\" }   # the chat's commit author; git's own identity when absent\n".to_string(),
    };
    format!(
        "# Mighty Crew configuration. Edit by hand; `crew doctor` checks it.\n\
         \n\
         [crew]\n\
         name = {crew_name:?}\n\
         \n\
         [chat]\n\
         repo = {repo_url:?}\n\
         path = \"docs/dev/chat.md\"\n\
         branch = {branch:?}\n\
         {identity}\
         \n\
         [agent]\n\
         command = \"claude\"\n\
         permission_mode = \"bypassPermissions\"      # one class for every role, so messages are delivered without a hold\n\
         msg_model = \"haiku\"                        # the model of the short headless session `crew msg` relays through\n\
         \n\
         # One table per role. `tree` is the role's own working tree; no two roles share one.\n\
         # A role may add `worktree_of` and `branch` so `crew start` can create a missing worktree.\n\
         [roles.{role_name}]\n\
         tree = {tree_text:?}\n\
         owns = \"everything, until more roles are added\"\n\
         \n\
         # [roles.product]\n\
         # tree = \"{tree_text}/.claude/worktrees/product\"\n\
         # worktree_of = {tree_text:?}\n\
         # branch = \"work/product\"\n\
         # owns = \"the product's code, tests, specs and docs\"\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: &str = "[crew]\nname = \"c\"\n[chat]\nrepo = \"r\"\n[roles.a]\ntree = \"/x/a\"\n[roles.b]\ntree = \"/x/a/b\"\nowns = \"inner\"\n";

    #[test]
    fn defaults_and_order() {
        let c = Config::parse(MIN, Path::new("/cfg")).unwrap();
        assert_eq!(c.chat.path, "docs/dev/chat.md");
        assert_eq!(c.chat.branch, "main");
        assert_eq!(c.agent.command, "claude");
        assert_eq!(c.agent.permission_mode, "bypassPermissions");
        assert_eq!(c.agent.msg_model, "haiku");
        assert_eq!(c.role_names(), vec!["a", "b"]);
        assert!(c.off_class_roles().is_empty());
        assert!(c.chat.identity.is_none());
    }

    #[test]
    fn the_deepest_tree_wins() {
        let c = Config::parse(MIN, Path::new("/cfg")).unwrap();
        assert_eq!(c.role_for(Path::new("/x/a/b/c")).unwrap().name, "b");
        assert_eq!(c.role_for(Path::new("/x/a/other")).unwrap().name, "a");
        assert!(c.role_for(Path::new("/y")).is_none());
        assert!(c.role("zz").unwrap_err().to_string().contains("the roles are: a, b"));
    }

    #[test]
    fn mistakes_are_named() {
        let err = |t: &str| Config::parse(t, Path::new("/cfg")).unwrap_err().to_string();
        assert!(
            err(&MIN.replace("[roles.b]\ntree = \"/x/a/b\"", "[roles.b]\ntree = \"/x/a\""))
                .contains("share the working tree")
        );
        assert!(err(&MIN.replace("[roles.b]", "[roles.B]")).contains("must be lowercase"));
        assert!(err("[crew]\nname = \"c\"\n[chat]\nrepo = \"r\"\n[roles]\n").contains("no roles"));
        assert!(err(&(MIN.to_string()
            + "[roles.d]\ntree = \"/d\"\ndispatcher = true\n[roles.e]\ntree = \"/e\"\ndispatcher = true\n"))
        .contains("at most one role is the dispatcher"));
        assert!(err("[chat]\nrepo = \"r\"\n[roles.a]\ntree = \"/a\"\n").contains("not a valid configuration"));
    }

    #[test]
    fn the_initial_configuration_parses() {
        let text = render_initial(
            "mycrew",
            "https://x/y.git",
            "main",
            Some(&Identity {
                name: "n".into(),
                email: "e".into(),
            }),
            "mycrew",
            "~/y",
        );
        let c = Config::parse(&text, Path::new("/cfg")).unwrap();
        assert_eq!(c.crew.name, "mycrew");
        assert_eq!(c.chat.identity.as_ref().unwrap().name, "n");
        assert_eq!(c.roles[0].name, "mycrew");
        assert!(Config::parse(&render_initial("a", "b", "c", None, "a", "/a"), Path::new("/cfg")).is_ok());
    }
}
