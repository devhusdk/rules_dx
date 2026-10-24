# `dx worktree`

```text
dx worktree add <branch> [--create=<ref>] [--output=path]
dx worktree release [<path>] [--force]
dx worktree list
dx worktree path <branch>
dx worktree prune [--apply]
dx worktree hook create|remove|guard
```

Keeps a small pool of warm checkouts, called slots, so a new worktree reuses
a slot that already has a Bazel server, output base and caches. Slots live at
`<cache>/dx/worktrees/<repo>/<n>`. `<cache>` is `$XDG_CACHE_HOME`, else
`$HOME/.cache`, else `%LOCALAPPDATA%`. Lease records live in
`<cache>/dx/worktrees/<repo>/.dx/`.

A bare `dx worktree` runs `list`.

Flags: `--create=<ref>` (add only), `--force` (release only), `--apply` (prune
only). A flag on the wrong subcommand exits `2`.
Scopes: none. `dx worktree` takes a subcommand and its arguments.
Reports: none.

`--output text|json|path`. `path` works with `add` only. Text prints the lines
above. `add` prints `dx worktree: <branch> is in slot <n> at <path>`.
`release` prints `dx worktree: released slot <n> at <path>`, or
`dx worktree: removed <path>` for an external worktree.

`--output=json` streams `command_started`, one `slot` event per slot, and
`command_finished`. `list` adds a `summary` event after the slots. `prune`
emits `slot` events with action `prune`, and action `prune_base` for output
bases.

`slot` fields: `action` (`add`, `release`, `list`, `path`, `prune`,
`prune_base`), `path`, `state` (`removed` for a released external worktree,
`external` for a pruned one), `server`, `disk_bytes`, and optional `slot`,
`branch`, `holder`, and `reason`.

`summary` fields: `slots`, `servers`, `servers_rss_bytes`, `disk_bytes`.

Exit codes: `0` success, `2` usage errors (unknown subcommand, wrong
arguments, a flag on the wrong subcommand, an unknown branch or ref,
`--create` on an existing branch, `release` of a slot another session holds,
and any other `dx` command in a slot leased to another session), `1`
operational failures (a branch already checked out elsewhere, a dirty or
unpushed slot or external worktree on `release` without `--force`, a busy pool lock, `path` finds
no checkout, `prune` finds work without `--apply`).

## Subcommands

`add <branch>` leases a slot, checks out `<branch>`, and prints where it is.
It picks the slot that last held the branch, then a slot with the same
`MODULE.bazel.lock`, then the most recently used. Below 10% free disk it takes
the oldest slot first. It reuses a free slot before it creates one. Files
matched by `.worktreeinclude` in the main checkout are copied in. `--create=<ref>`
starts a new branch from `<ref>`. Without it, `<branch>` must exist locally or
as `origin/<branch>`. `--output=path` prints only the path.

`add` refuses when the branch is already checked out in a slot held by another
session or in a worktree outside the pool, when `--create` names a branch that
exists, and when `<branch>` does not exist.
`add` takes over a held slot, including one that holds `<branch>`, only when
its holder process is gone, the tree is clean, and no commit is unpushed. It
does not wait. Otherwise it refuses and says which condition fails. The holder
is the nearest ancestor of `dx` that is not a shell. `add` also deletes free
slots idle for 30 days.

`release [<path>]` frees the slot at `<path>`, or the slot you are in. It
stops the slot's Bazel server and detaches the checkout. The branch and its
commits stay in the repository. It refuses a dirty tree or unpushed commits
unless you pass `--force`; uncommitted changes are then wiped by the next `add`
of that slot. Only the holder can release a held slot, unless it is
reclaimable or you pass `--force`.

`release <path>` on an `external` worktree of this repository, such as an old
`.claude/worktrees/<task>` checkout, stops its Bazel server, removes the
checkout and its output base, and keeps the branch. It refuses a dirty tree or
unpushed commits unless you pass `--force`, which removes it anyway. The main
checkout is never removed.

`list` prints one line per slot and a totals line:

```text
1	feature-x	session 7f3a pid 4121	held	server	1.2 GiB	/home/me/.cache/dx/worktrees/app/1
2	(detached)	-	free	no server	900.0 MiB	/home/me/.cache/dx/worktrees/app/2
1 bazel server(s) running, 2.1 GiB resident, 2.1 GiB across slots
```

The columns are slot, branch, holder, state, server, disk use, and path.
`state` is `held`, `free`, `reclaimable` (holder gone, and the reclaim rule
above would allow taking it), or `external` (a worktree outside the pool). A
held slot whose holder is gone but which has uncommitted changes or unpushed
commits shows why after `held`, such as `held (holder gone, commits are not
pushed)`.

`path <branch>` prints the path of the checkout that holds `<branch>`, and
fails with exit `1` when none does.

`prune` lists free slots idle for 30 days, `external` worktrees that are clean,
fully pushed, not locked, without a running Bazel server, and on a pushed branch
that is merged into the default branch or whose remote branch is deleted, and Bazel output bases of removed worktrees,
including checkouts removed from `.claude/worktrees`. `--apply` removes the
external worktrees with their output bases and keeps the branches. It deletes nothing without `--apply`. It exits `1` when something
would be pruned and `--apply` is absent, and `0` when there is nothing to prune.

## Hooks

`hook create`, `hook remove`, and `hook guard` read one JSON object from stdin
for Claude Code hooks.

- `create` reads `name` and `cwd`. It leases a slot on branch
  `worktree-<name>` (created from the default branch when new) and prints only
  the path. Subagent worktrees get no `remove` hook, so `create` first frees
  the session's earlier `worktree-*` slots that are at least a minute old,
  clean, fully pushed, and not in use. A slot is in use while a Bazel server
  runs in it or a live process has its working directory inside it. On
  platforms where `dx` cannot read process working directories (Linux reads
  `/proc`, macOS runs `lsof`), `create` frees nothing. A slot freed this way
  goes to a later `create`, never the one that freed it.
- `remove` reads `worktree_path` and releases that slot. Paths outside the
  pool are ignored.
- `guard` reads `tool_input.command`. It exits `2` with a message on stderr
  when the command runs `git worktree add`, `remove`, or `prune`, and `0`
  otherwise.

Commit `.claude/settings.json`. The hooks run the repo launcher, so `dx` does
not need to be on `PATH`:

```json
{
  "hooks": {
    "WorktreeCreate": [{"hooks": [{"type": "command", "command": "\"${CLAUDE_PROJECT_DIR:-.}\"/tools/dx worktree hook create"}]}],
    "WorktreeRemove": [{"hooks": [{"type": "command", "command": "\"${CLAUDE_PROJECT_DIR:-.}\"/tools/dx worktree hook remove"}]}],
    "PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "\"${CLAUDE_PROJECT_DIR:-.}\"/tools/dx worktree hook guard"}]}]
  }
}
```

`dx init --apply` writes this file when it is absent and never overwrites it. If
`.claude/settings.json` exists, it prints the missing entries. It also writes
the launcher and the agent commands block in `AGENTS.md`; see
[`dx init`](init.md). `dx status` reports the `agents` check as `warn` when a
hook is missing.

## Leases

The holder is the agent session (`CLAUDE_CODE_SESSION_ID` or
`CODEX_THREAD_ID`) and the parent process. Any other `dx` command run inside a
slot leased to a different session exits `2` and names the holder. `dx status`
inside a slot shows the lease. `worktree`, `version`, and `completion` always
run.

## Examples

```sh
bazel run @rules_dx//:dx -- worktree add feature-x --create=origin/main
bazel run @rules_dx//:dx -- worktree add feature-x --output=path
bazel run @rules_dx//:dx -- worktree release
bazel run @rules_dx//:dx -- worktree list --output=json
bazel run @rules_dx//:dx -- worktree prune
bazel run @rules_dx//:dx -- worktree prune --apply
```
