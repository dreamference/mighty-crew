# Mighty Crew

A crew of coding-agent sessions on one codebase: one role per working tree, a shared board every role writes to and reads from, and messages between roles. By Dreamference, the makers of [Mightling](https://github.com/dreamference/mightling).

Running one agent session per concern (product, engine, operations) is the natural way to use more than one at once. Done by hand, two writers end up in one checkout, no session knows what the others decided, messages between sessions are held and expire, and a reboot forgets which directory belonged to whom. Mighty Crew is the small tool that keeps those four things straight.

```
crew start product        # the product role's agent, in its own tree, with the board in front of it
crew note "release branch prepared; cut after night 2"
crew msg engine "the API spec moved; section 7 is yours"
crew status               # every role: tree, branch, head, running or idle, last note
```

**Status:** an MVP specification and a working shell prototype, nothing more yet. The specification is [specs/DREAMFERENCE_MIGHTY_CREW.md](specs/DREAMFERENCE_MIGHTY_CREW.md); the three scripts in [prototype/](prototype/) are the behaviour it was written from, run by hand on one machine with four roles. The MVP is one Rust binary, `crew`, that runs Claude Code sessions; other agents follow.

**License:** AGPL-3.0, as Mightling.
