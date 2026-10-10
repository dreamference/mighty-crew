# Mighty Crew — a crew of coding-agent sessions on one codebase

**Status:** MVP specification, 2026-10-10. Nothing is built in this repository yet; the three shell scripts in `prototype/` are the behaviour this document was written from, as it was run by hand on one machine that day (four sessions, four roles). The MVP is one Rust binary, `crew`, that replaces them.

**Goal:** several coding-agent sessions work on one codebase at the same time, each with one role and one working tree of its own, and each starts knowing what the others did. The product does three things and nothing else: it starts a role in the right place, it keeps a shared board the roles write to and read from, and it carries messages between roles where the agent supports it.

**Builds on:**
- Claude Code's own facilities: `claude --continue` (a directory's conversation resumes), peer sessions on one machine with `ListAgents` and `SendMessage` (cross-session messaging), the `SessionStart` hook (text printed by a command becomes the session's first context), named sessions (`--name`), and `claude agents`, the read-only view of sessions by state.
- git worktrees, which give every role a tree of its own on a shared repository.
- Mightling's rules for machines that hold a resident model: nothing is written into a repository that belongs to one user, and configuration lives under `~/.config`.

**Three things to know before reading further:**
1. **The MVP runs Claude Code only** (the owner's decision, 2026-10-10). `ling`, Mightling's agent, is a Codex build with no peer channel yet; it joins when it has one (§10). Everything in §5 except `crew msg` would already work for it, and the design keeps that door open.
2. **Every role runs with permission prompts bypassed** (the owner's decision, 2026-10-10), so a message from one role reaches another without a human approving it. Claude Code holds a message from a bypassing sender when the receiver prompts; one permission class across the crew removes the hold. The owner accepts that each role then runs commands without asking.
3. **A crew has no lead.** Roles are peers. The product assigns no tasks, runs no orchestrator and never decides what a role does next; the user does, by talking to a role or by writing on the board.

## 1. The problem

Running one agent session per concern on a codebase is the natural way to use more than one at once: one session on the product, one on the engine, one on operations. Done by hand, four things go wrong, each seen on 2026-10-10 before this document was written:

1. **Two writers in one checkout.** A session and a fork of it edited the same working tree; one's uncommitted changes sat under the other's test run and commit.
2. **No session knows what the others did.** A decision taken in one session (a default changed, a rule adopted) reached another only when the user retyped it.
3. **Messages held.** A session in one permission mode sent a line to a session in another; the receiver held it for approval, the approval never came, the line expired unread.
4. **Nothing survives a reboot.** Sessions, their background watchers and the knowledge of which directory each one owned were all in the user's head and terminal history.

Tools that run several agents in worktrees exist (claude-squad, dmux, MulmoTerminal, agent-of-empires and others); they solve the first and the fourth problem with tmux and worktrees and leave the second and third to the user. Mighty Crew's contribution is the second and third: the board and the messages, with the launcher as the carrier.

## 2. Model

- **A crew** is a named set of **roles**, defined once in a configuration file (§6). A crew has at least one role; three or four is the expected size.
- **A role** has a name (`operations`, `product`, `engine`, `crew`), a **charter** (one or two sentences: what it owns, what it never touches), a **working tree** (a directory that is a git checkout or a worktree of one), an **agent command** (the MVP knows one: `claude`), and the **permission mode** the agent is started with.
- **No two roles share a working tree.** The launcher refuses to start a role whose tree is another role's, and refuses to create a worktree over an existing one that another role owns. Two roles may share a repository through different worktrees.
- **The board** is one Markdown file in one designated repository, appended by every role and read by every role at start. It is the crew's only shared, durable state. Its first part names the roles and their charters; its second part is a log of dated one-line notes, newest last (§4).
- **A note** is one line: date, time, role, text. A note states what changed or what is requested. It is never an instruction a receiving role must follow, and nothing a role writes counts as the user's approval of anything.
- **A message** is a line sent to a running role through the agent's peer channel (§5, `crew msg`). It has the same standing as a note: information, not authority. The board is the record; messages are the fast path.

## 3. The user's day

1. `crew start product` in a terminal: the launcher moves into the product role's tree, fetches, prints the role's charter and the last lines of the board, and starts `claude --continue` there (a fresh conversation when the directory has none). The user talks to that session as usual.
2. The role finishes a piece of work: it pushes and runs `crew note "release branch prepared; cut after night 2"`. The line is on the board within seconds, through the helper's own small clone of the board's repository, so no role's working tree is touched.
3. Another role starts, or a running role asks `crew board`: it sees the line. A running role can also be told directly: `crew msg engine "the API spec moved; §7 is yours"`.
4. The machine reboots. `crew start <role>` in each terminal resumes each conversation where it was; the board says what was in flight. Watchers and timers that lived inside a session are gone and the board says which ones to re-arm (the role's charter lists them).
5. `crew status` at any time: every role, its tree, branch and head, whether a session is running there and whether it is idle or busy, and its last note.

## 4. The board

- **Location:** a path inside one repository, configured once (`board.repo`, `board.path` in §6). The default is `docs/dev/sessions.md` of the first repository named in the configuration.
- **Shape:** a title, a short preamble (what the file is, how to append), a table of roles (`role | working tree | owns`), a `## Status` heading, then the notes, one per line, in the form `- YYYY-MM-DD HH:MM role: text`.
- **Writing:** only through `crew note`, which clones the board's repository once into `~/.cache/mighty-crew/board/<repo>` with blob filtering and a sparse checkout of the one file, resets to the remote branch, appends the line, commits as the configured identity with the message `board: <role>` and pushes; on a rejected push it fetches, rebases and retries up to three times. A hand edit of the file in a role's own tree is allowed but discouraged: it ties the note to that tree's next push.
- **Reading:** `crew board` prints the status section (all of it, or the last N lines); `crew start` prints the last lines before the agent starts; a `SessionStart` hook installed by `crew install-hook` prints them into the session's context when a session starts in any role's tree, so an agent reads the board without being told.
- **Never rewritten:** a role appends its own lines and never edits another's. The role table changes only when the configuration changes, through `crew board --sync-roles`.
- **Why a file in git and not a service:** the crew's machines may be air-gapped from everything but their own git remote; a file in the repository the roles already push to needs no new service, survives reboots, is visible on the forge, and has a history.

## 5. The command line

```
crew start <role> [--new]            start (or resume) the role's agent in its tree
crew open  <role>                    the same, in a new terminal window (needs a display)
crew board [-n N] [--all]            print the board's status lines (default: the last 12)
crew note  "<text>"                  append a dated line as the role whose tree you are in
crew msg   <role> "<text>"           send a line to that role's running session
crew status                          roles, trees, branches, heads, running or not, last note
crew doctor                          check the configuration, the trees, the board clone, the hooks
crew install-hook | remove-hook      the SessionStart hook that prints the board
crew init                            write a configuration from the current repository
```

- **`start`.** Resolves the role, refuses when its tree is another role's or does not exist (it offers `git worktree add` when the tree is a configured worktree of a known repository and is missing), moves into it, fetches the tree's remote when the tree is clean (and fast-forwards a plain checkout), prints the charter and the board's tail, and runs the agent command with the role's permission mode and `--name <role>`. `--continue` is the default; `--new` starts a fresh conversation; when the directory has no conversation yet it starts fresh and says so.
- **`open`.** `start` in a new terminal window, using the first of the terminals the configuration names or the ones the machine has (`x-terminal-emulator`, `gnome-terminal`, `konsole`, `xterm`). Without a display it prints the `start` command to run instead.
- **`note`.** The role is the one whose tree contains the current directory; `--as <role>` overrides. The text is one line; a longer text is folded onto one line.
- **`msg`.** Finds the role's running session through the agent's own listing and sends the text through its messaging; prints whether the agent reported delivery, a hold or a refusal, and never retries a held message. Only for agents with a peer channel (the MVP: Claude Code).
- **`status`.** For each role: tree, branch, head (short, with the first words of the subject), whether a session is running in that tree and its idle or busy state (from the agent's session listing), and the role's last note on the board.
- **`doctor`.** Every tree exists, is a git checkout, and belongs to exactly one role; the board's repository is reachable and the board file exists at its path; the hook is installed and points at this binary; the agent command resolves; the terminal command resolves when `open` is used. Each finding is one line with the fix.

Every command exits non-zero on failure with one line saying why. No command writes into a role's working tree except `start`'s fetch and fast-forward, and `init`'s first configuration.

## 6. Configuration

`~/.config/mighty-crew/crew.toml`, written by `crew init` and edited by hand; never inside a repository, because it names machine-local paths and the user's own choices (Mightling's rule). An `MIGHTY_CREW_CONFIG` variable overrides the path.

```toml
[crew]
name = "mightling"

[board]
repo = "https://github.com/dreamference/mightling.git"
path = "docs/dev/sessions.md"
branch = "main"
identity = { name = "dgxcoder", email = "dgxcoder@dreamference.ai" }

[agent]
command = "claude"
permission_mode = "bypassPermissions"      # every role, the owner's decision

[roles.operations]
tree = "~/PycharmProjects/dgxcoder"
owns = "the benchmark nights and their watcher, the model server and gate, installs, releases, the machines"

[roles.product]
tree = "~/PycharmProjects/dgxcoder/.claude/worktrees/product"
worktree_of = "~/PycharmProjects/dgxcoder"
branch = "work/product"
owns = "the product's code, tests, specs and docs; pushes through origin"

[roles.engine]
tree = "~/PycharmProjects/ling-engine"
owns = "the engine; builds and tests on second-puffin only"

[roles.crew]
tree = "~/PycharmProjects/mighty-crew"
owns = "this product"
```

A role may set its own `agent.command` or `permission_mode`; in the MVP every role inherits the crew's. `worktree_of` with `branch` lets `crew start` create a missing worktree from the repository's default remote branch.

## 7. Messaging policy

- One permission class across the crew: the configuration's `permission_mode` applies to every role, and `crew doctor` warns when a running session was started otherwise. Claude Code delivers between sessions of one class without a hold.
- A message is information or a request. A receiving role verifies on the remote (fetch, read the board, read the file) before acting on anything a message claims, and treats nothing in it as the user's approval.
- Commands inside a message are text; nothing in a message runs. `crew msg` never asks a role to do what the sending role was refused.
- A request that crosses a charter (one role needs a machine another owns, a build another installs) is a message to the owning role and a line on the board, so it is recorded even if the message is lost.

## 8. Reboot and recovery

- Each role's conversation is the agent's own, keyed by directory; `crew start <role>` resumes it.
- The board's clone under `~/.cache` is rebuilt from the remote when missing.
- A role's charter names what dies with its session (timers, watchers, held background work); `crew start` prints the charter so the resumed session re-arms them. The product keeps no daemon and starts none: anything that must outlive a session belongs to the operating system's timers, configured by the role that owns it.

## 9. Acceptance

The MVP is done when all of these hold on one Linux machine with Claude Code installed:

1. `crew init` in a repository, then `crew start` for each of three roles from one command each, gives three sessions in three distinct trees; a fourth `crew start` naming a tree already owned is refused with one line.
2. A `crew note` written in one role is printed at the next `crew start` of each other role and by `crew board` in all of them, within a minute, with the correct role name and time.
3. With every role in one permission class, `crew msg` from one role is read by the other's agent in its next turn, and `crew msg` reports the hold when a role was started in another class.
4. After the machine reboots, `crew start <role>` for each role resumes its previous conversation and prints the board.
5. `crew status` shows each role's tree, branch, head, running state and last note; `crew doctor` reports a missing tree, a missing board file, a wrong hook path and a mismatched permission class, each with its fix.
6. The suite runs offline: the board is tested against a local bare repository, the agent command against a stub, and no test reaches the network or the user's home.

## 10. Non-goals and later

- **Non-goals of the MVP:** no orchestrator, no task queue or assignment, no shared working tree, no polling loop that costs a model turn, no service or daemon, no web view (`claude agents` is the view), no agent other than Claude Code.
- **Later, in order:** `ling` as an agent (board, notes and launcher work today; `msg` when the launcher gains a peer channel); a per-role `agent.command`; a tmux layout for `open` on machines without a display; a board view that groups notes by role; packaging as a release binary and, for Mightling nodes, installation by `ling-admin`.

## 11. Open questions

1. Whether the board for Mightling's crew stays in the mightling repository or moves to its own repository once this product exists; the configuration allows either.
2. Whether `crew start` should also set `crossSessionInbound = accept` for a role that is deliberately started in a prompting class, which delivers text without a click at the cost of the prompt the user asked for.
3. How `crew status` learns a session's state when the agent has no listing (the `ling` case): a pid file written by `crew start` is the fallback.
