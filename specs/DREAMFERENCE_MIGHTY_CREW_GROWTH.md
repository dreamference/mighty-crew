# Mighty Crew — how it spreads

**Status:** proposals, 2026-10-10. Nothing here is built, and nothing here is in the MVP's acceptance ([DREAMFERENCE_MIGHTY_CREW.md](./DREAMFERENCE_MIGHTY_CREW.md) §9, "the MVP" below). Each item is a dated proposal with the line that says it is done; an item moves to the MVP document when it is built and its line holds.

**Goal:** a tool for several agents is seen by nobody but its user, so it spreads through what a crew leaves in public: the board on the forge, the digest in a pull request, the repository that carries its own crew. Every item names that public artifact. None adds a service, a daemon, telemetry or a model turn, and nothing leaves the machine that the user did not ask to send: the MVP's non-goals (§10) hold here too.

**Builds on:** the board as a Markdown file in git, rendered by the forge (MVP §4); the rule of MVP §6 that configuration naming machine-local paths lives in `~/.config`, which leaves a repository free to carry what is not machine-local (§2 below); the offline test suite with its stub agent (MVP §9.6).

## 1. The board names its keeper

The preamble `crew` writes above the role table (MVP §4) ends with one line: `Kept with Mighty Crew (https://github.com/dreamference/mighty-crew).` A board on a forge is read by everyone who reads the repository, and the dated, role-by-role hand-offs are the product's demonstration; the line says where it came from. `crew board --sync-roles --plain` leaves the line out, and a preamble edited by hand is never rewritten unless `--sync-roles` is asked for.

Done when a fresh board carries the line and one written with `--plain` does not.

## 2. A repository carries its crew

Role names, charters, branches and worktrees relative to the repository are not machine-local; trees are. A repository may ship `.crew/roles.toml` with exactly those fields:

```toml
[roles.product]
owns = "the product's code, tests, specs and docs"
worktree = ".claude/worktrees/product"     # relative to the repository root
branch = "work/product"
```

A role with no `worktree` is the clone itself, and there is at most one such role. The file is written by the repository's maintainers, as they write the board's path, never by the tool: `crew init` in a clone only reads it, asks for nothing but the main tree, and writes the user's configuration (MVP §6, which stays in `~/.config` and keeps every path and every choice of the user's own; the rule that the tool writes nothing into a repository stands). A contributor's clone is crew-ready with `crew init`, a fork inherits the crew, and the file is the first thing a reader of the repository learns about how it is worked on.

Done when `crew init` in a clone that ships the file produces a configuration whose roles match the file and whose trees are under the clone.

## 3. The digest is what gets pasted

`crew digest [--since <date>] [--role <role>]` renders the board's notes as a Markdown summary (one heading per role, the notes in clock order; on a machine that has the role's tree, also the commits of that tree between the times of the role's first and last note, from `git log`, since a note carries no head) fit for a pull-request description, a release note or a post; `--pr` puts it in the current branch's pull request through `gh` when present. The thing people paste is the thing other people see; the digest ends with the keeper line of §1 exactly when the board carries it.

Done when a digest of the board of MVP §9.2 lists every role that wrote a note and nothing else.

## 4. First contact in a minute, offline

`crew demo` creates a throwaway repository under the cache with a local bare remote and two roles, writes a configuration for it beside them with a stub agent of its own (a script beside the configuration that answers `agents --json` with `[]` and ignores everything else, the shape the test suite's stub has, MVP §9.6), and prints three commands, each prefixed with `MIGHTY_CREW_CONFIG=<that path>` so they bind to the demo and not to the user's crew, and run from any directory: `crew note --as <role> "<text>"`, `crew board`, `crew status`. Trying the product needs no Claude Code session and no network. Installation is `cargo install mighty-crew` and a release binary (MVP §10).

Done when the three printed commands, run in order in a fresh shell, end with `crew board` printing the line `crew note` wrote, offline.

## 5. A view for a screen

`crew watch [--every <seconds>]`: the `status` table redrawn on each fetch of the board's clone (default every 30 seconds; a fetch, never a model turn) and on each session listing, with the last three notes under it. This is the view that goes into a screencast, and the one left on a second monitor.

Done when a note written in another terminal appears in the view within one interval.

## 6. Numbers, not a dashboard

`crew stats [--since <date>]`: notes per role, per day and per hour of the day, and the first and last note of each role, as one screen of plain text or Markdown (`--md`) for the digest. No service and no upload; every number comes from the board's lines and nothing else, because the start record of MVP §5 `doctor` keeps one file per role and counts nothing. Sessions started and messages sent join when `crew start` and `crew msg` keep an append-only log under the cache, a later item.

Done when the counts equal those of the board's lines.

## 7. A crew that reads another's board

`[[board.also]]` tables, each with `repo`, `path` and `branch` as in `[board]`, let `crew board` and the hook tail more than one board, newest last across them, each line prefixed with its crew's name. The other boards are read only, and `crew note` writes to `board.repo` alone, so MVP §2 still holds: one board is the crew's own durable state. The first case is the mightling crew and the mighty-crew crew (MVP §11.1): one is built with the other, and each sees what the other decided.

Done when a note on either board is printed at the next `crew start` of a role in the other crew.

## 8. `crew up`: a clone becomes a crew in one command

(2026-10-10.) crewAI spread on a sixty-second path: `crewai create crew` writes the roles, `crewai run` runs them, and an examples repository gives something to clone. This document has the first half (§2, a repository ships `.crew/roles.toml`) and the offline first contact (§4), and nothing that plays the part of `run`: today a stranger who clones a repository that carries a crew still runs `crew init`, then `crew start` once per role in a terminal of their own. `crew up` is that one command.

`crew up [--new]`: for every role of the configuration, in the order written, creates the worktree when it is missing and configured (`worktree_of` and `branch`, as `crew start --create` does), then opens every role at once and attaches:

- with `tmux` on `PATH`: one tmux session named after the crew (`crew.<name>`), one window per role, each running `crew start <role>` (so the live-session refusal, the fetch, the charter and the board of MVP §5 all apply), then `tmux attach`; a second `crew up` finds the session and attaches without starting anything;
- with a display and no `tmux`: one terminal window per role, through `open`'s path (MVP §5);
- with neither: the `crew start` lines to run, one per role, and exit 0.

`up` opens sessions and assigns nothing: a crew has no lead (MVP, "three things to know"), and the charters and the board do the rest. There is no `crew down`: the product never ends a session (MVP §10.4); the user closes windows, and `crew note` is the hand-off. `up` adds no dependency: `tmux`, when present, is run through `std::process` like git and the agent, and the layout is the one MVP §10.9 already names.

The public artifact is one line in the README of any repository that carries a crew:

```bash
git clone https://github.com/<owner>/<repo>.git && cd <repo> && crew init && crew up
```

That line is the loop: a reader of the board on the forge (§1) sees the crew, clones the repository, and is in it in a minute. It is also what a screencast shows, before `crew watch` (§5) exists. Mixed crews, where one role runs Claude Code and another runs `ling` or any agent with a launcher (MVP §10.1), make the same line worth more later; `up` does not depend on them.

Effort: a day or two on the MVP's own pieces (`start`, `--create`, `open`, the session listing) and `tmux new-session`, `new-window`, `has-session` and `attach`. Sequenced after the decisions queue (MVP §10.10), which the first day's evidence put first.

Done when a fresh clone of a repository that ships `.crew/roles.toml`, followed by `crew init` and `crew up`, gives one tmux window per role, each session named after its role and starting with the board in its context, in under a minute from the clone to the last session's prompt, measured on one machine with the agent's time counted.

## 9. Order

The order is the order of effect for the cost: §1 and §2 are a line and a file; §3 and §4 are an afternoon each; §8 is a day or two and is the first thing a stranger runs, so it comes right after §2; §5, §6 and §7 wait for a crew that asks for them. A proposal that is built moves to the MVP document with its date and its measured result.
