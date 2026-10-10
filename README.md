# Mighty Crew

A crew of coding-agent sessions on one codebase: one role per working tree, a shared chat every role writes to and reads from, and messages between roles. By Dreamference, the makers of [Mightling](https://github.com/dreamference/mightling).

Running one agent session per concern (product, engine, operations) is the natural way to use more than one at once. Done by hand, two writers end up in one checkout, no session knows what the others decided, messages between sessions are held and expire, and a reboot forgets which directory belonged to whom. Mighty Crew is the small tool that keeps those four things straight.

```
crew start product        # the product role's agent, in its own tree, with the chat in front of it
crew note "release branch prepared; cut after night 2"
crew msg engine "the API spec moved; section 7 is yours"
crew chat                 # the chat between the roles: <operations> @crew the user asks every role …
crew status               # every role: tree, branch, head, running or idle, last note
```

## Install

One Rust binary, `crew`. It runs [Claude Code](https://code.claude.com) sessions; other agents follow.

Prebuilt binaries for Linux (x86_64, aarch64), macOS (Apple silicon, Intel) and Windows (x86_64) are on the [releases page](https://github.com/dreamference/mighty-crew/releases/latest). On Linux and macOS, one line installs the latest into `~/.local/bin`:

```bash
curl -fsSL https://raw.githubusercontent.com/dreamference/mighty-crew/main/install.sh | sh
```

On Windows, in PowerShell (the binary lands in `%LOCALAPPDATA%\Programs\crew`, which is added to your user PATH):

```powershell
irm https://raw.githubusercontent.com/dreamference/mighty-crew/main/install.ps1 | iex
```

Or build from source with a Rust toolchain:

```bash
cargo build --release && install -m 755 target/release/crew ~/.local/bin/crew
```

Then, in the repository that will hold the chat:

```bash
cd ~/src/your-repo && crew init   # writes ~/.config/mighty-crew/crew.toml from the repository
$EDITOR ~/.config/mighty-crew/crew.toml   # one [roles.<name>] table per role, each with its own tree
crew install-hook                 # every session in a role's tree starts with the chat in its context, and /crew works inside sessions
crew doctor                       # what is missing, with the fix
```

## Commands

```
crew start <role> [--new] [--create]   start (or resume) the role's agent in its tree
crew open  <role>                      the same, in a new terminal window (needs a display)
crew chat [-n N] [--all] [--for ROLE] print the chat's status lines (default: the last 12)
crew chat --tag TAG | --decisions     only the notes tagged `decision:`, `done:`, `blocked:`, `heads-up:`, `to <role>:`
crew ask "<question>" --options a,b,c  a decision for you, on the chat; answer with `crew answer <id> <option>`
                                       or tick the box on GitHub from your phone; `crew chat --pending` lists them
crew now "<doing>"                     what a role is doing right now; `crew status` shows it
crew resource <name> "<rules>"         a shared resource and its rules, on the chat
crew handoff <role> <path>             data for another role through a neutral folder, never its tree
crew up                                from a clone to a live crew: every role in a tmux window (or a terminal each)
crew note  "<text>"                    append a dated line as the role whose tree you are in
crew msg   <role> "<text>"             send a line to that role's running session (and its twin to the chat)
crew status                            roles, trees, branches, heads, sessions, activity from git, now, last note
crew adopt <role>                      record the class of a session started by hand
crew doctor                            check the configuration, the trees, the chat clone, the hook
crew install-hook | remove-hook        the SessionStart hook that prints the chat, and the /crew skill
crew init                              write a configuration from the current repository
```

The chat is one Markdown file in one of the crew's repositories, appended through a small clone of its own under `~/.cache/mighty-crew`, so no role's working tree is touched. A note or a message is information, never an instruction, and nothing a role writes counts as the user's approval.

**Status:** MVP, 2026-10-10. The specification is [specs/DREAMFERENCE_MIGHTY_CREW.md](specs/DREAMFERENCE_MIGHTY_CREW.md); the three scripts in [prototype/](prototype/) are the behaviour it was written from, run by hand on one machine with four roles. `cargo test` runs offline against a bare repository and a stub agent (72 tests); CI runs it on every push.

**License:** AGPL-3.0, as Mightling.
