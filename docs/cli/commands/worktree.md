# `dx worktree`

```text
dx worktree add <branch> [--create=<ref>] [--force] [--output=path]
dx worktree release [<path>] [--force]
dx worktree list [--disk]
dx worktree path <branch>
dx worktree prune [--apply]
dx worktree hook create|remove|stop|guard
```

Keeps a small pool of warm checkouts, called slots, so a new worktree reuses
a slot that already has a Bazel server, output base and caches. Slots live at
`<cache>/dx/worktrees/<repo>/<n>`. `<cache>` is `$XDG_CACHE_HOME`, else
`$HOME/.cache`, else `%LOCALAPPDATA%`. Lease records live in
`<cache>/dx/worktrees/<repo>/.dx/`.

A bare `dx worktree` runs `list`.

Flags: `--create=<ref>` (add only), `--force` (add and release), `--disk`
(list only), `--apply` (prune only). A flag on the wrong subcommand exits `2`.
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

When `git fetch origin` fails, `add` still uses local refs. Text prints
`dx worktree: cannot fetch origin, using local refs: <error>` before the slot
line. JSON emits a `notice` event with level `warning` and code `fetch_failed`
after `command_started`.

When free disk is low, `add` and `release` print
`dx worktree: low disk: <free> free; removed <what>; <free> free now` before
the slot line, and end with `; run dx worktree prune --apply` while disk stays
low. JSON emits a `notice` event with level `warning` and code `disk_low`
before the `slot` event.

`slot` fields: `action` (`add`, `release`, `list`, `path`, `prune`,
`prune_base`), `path`, `state`, `server`, and optional `slot`, `branch`,
`holder`, `reason`, and `disk_bytes`. `disk_bytes` is set for `prune` and
`prune_base` events other than `kept`, and for `list` with `--disk`.

`state` values:

- `held`: `add`, `path`, and `list` for a leased slot.
- `free`: `release` and `prune` of a slot, and `list` for an unleased slot.
- `reclaimable`: `list` for a held slot that `add` may take over.
- `external`: `list` and `prune` for a worktree outside the pool.
- `removed`: `release` of an external worktree.
- `free`: also `prune_base` for the output base of a free slot, with its
  `slot`.
- `kept`: `prune --apply` for a worktree it skipped, and `prune_base` for an
  orphaned output base it keeps, with its `reason`.
- `orphaned`: `prune_base` for an output base whose workspace is gone.

`summary` fields: `slots`, `servers`, `servers_rss_bytes`, plus `disk_bytes` and
`disk_cache_bytes` with `--disk`.

Exit codes: `0` success, `2` usage errors (unknown subcommand or hook name,
wrong arguments, a flag on the wrong subcommand, an unknown `--create` ref,
`--create` on an existing branch, `release` of a slot another holder holds
without `--force`, and any other `dx` command in a slot leased to another
session), `1` operational failures (a branch already checked out elsewhere or
held by another holder without `--force`, a dirty or
unpushed slot or external worktree on `release` without `--force`, a busy pool lock, `path` finds
no checkout, `prune` finds work without `--apply`, no default branch to start
a new branch from, and any `hook create`, `hook remove`, or `hook stop`
failure).

## Subcommands

`add <branch>` leases a slot, checks out `<branch>`, and prints where it is.
It picks the slot that last held the branch, then a slot with the same
`MODULE.bazel.lock`, then the most recently used. When disk is low, under
20 GiB and under 10% free, it takes the oldest slot first. Then it deletes
orphaned output bases, then the output bases of free slots, least recently
used first, until disk is no longer low. It reuses a free slot before it creates one. Files
matched by `.worktreeinclude` in the main checkout are copied in. `--create=<ref>`
starts a new branch from `<ref>`. Without it, `add` uses the local `<branch>`,
then `origin/<branch>`, and otherwise starts `<branch>` from the default branch
(`origin/HEAD`, else `origin/main`, else `origin/master`). `--output=path` prints
only the path on stdout and writes the `dx worktree: ...` warning lines to
stderr.

`add` refuses when the branch is already checked out in a slot held by another
holder or in a worktree outside the pool, and when `--create` names a branch
that exists. `add` returns your own slot for
`<branch>` as is. `add --force` hands a slot that holds `<branch>` for another
holder to you as is, without resetting it.
`add` takes over a held slot, including one that holds `<branch>`, only when
its holder is gone, the tree is clean, and no commit is unpushed. It
does not wait. Otherwise it refuses and says which condition fails.
Each refusal ends with the next step, such as `pass --create=<ref>`,
`run the command again`, or `pass a path from dx worktree list`. `add` also
deletes free slots idle for 30 days.

`release [<path>]` frees the slot at `<path>`, or the slot you are in. It
stops the slot's Bazel server and detaches the checkout. The branch and its
commits stay in the repository. It refuses a dirty tree or unpushed commits
unless you pass `--force`; uncommitted changes are then wiped by the next `add`
of that slot. Only the holder can release a held slot, unless it is
reclaimable or you pass `--force`. Another agent in the same session is
another holder. When disk is low, `release` deletes orphaned output bases,
then the output bases of free slots, least recently used first, until disk is
no longer low.

`release <path>` on an `external` worktree of this repository, such as an old
`.claude/worktrees/<task>` checkout, stops its Bazel server, removes the
checkout and its output base, and keeps the branch. It refuses a dirty tree or
unpushed commits unless you pass `--force`, which removes it anyway. The main
checkout is never removed.

`list` prints one line per slot and a totals line:

```text
1	feature-x	session 7f3a agent a91c pid 4121	held	server	-	/home/me/.cache/dx/worktrees/app/1
2	(detached)	-	free	no server	-	/home/me/.cache/dx/worktrees/app/2
1 bazel server(s) running, 2.1 GiB resident
```

The columns are slot, branch, holder, state, server, disk use, and path. The
disk column shows `-` unless you pass `--disk`, which measures each slot and
its output base and adds the total to the last line, followed by the size of
the shared disk cache (`0 B` when it does not exist). It is slow on a large
pool.

```text
$ dx worktree list --disk
1	feature-x	session 7f3a agent a91c pid 4121	held	server	1.2 GiB	/home/me/.cache/dx/worktrees/app/1
2	(detached)	-	free	no server	900.0 MiB	/home/me/.cache/dx/worktrees/app/2
1 bazel server(s) running, 2.1 GiB resident, 2.1 GiB across slots, 3.4 GiB disk cache
```

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
including checkouts removed from `.claude/worktrees`. When disk is low, it also
lists the output bases of the other free slots, least recently used first, as
`output base of free slot <n>, <size>: <dir>`. `--apply` stops each slot's
Bazel server and deletes those output bases; the slots stay. `--apply` removes the
external worktrees with their output bases and keeps the branches. It deletes nothing without `--apply`. It exits `1` when something
would be pruned and `--apply` is absent, and `0` when there is nothing to prune.

`--apply` checks each external worktree again under the pool lock. One that
became locked, got a Bazel server, uncommitted changes, or unpushed commits is
kept and printed as `kept external worktree <branch> (<reason>): <path>`, with
reason `locked`, `server`, `dirty`, or `unpushed`. JSON reports it with state
`kept` and that `reason`. It still exits `0`.

An output base whose workspace is gone but that a Bazel server still uses is
kept and printed as `kept output base of removed <workspace> (server): <dir>`.
JSON reports it as `prune_base` with state `kept` and reason `server`.

## Shared disk cache

The main checkout and every slot share one Bazel disk cache at
`<cache>/dx/disk-cache`, so a branch built in one checkout is not rebuilt in
another. `dx build`, `test`, `coverage`, `run`, and `deploy` add
`--disk_cache=<cache>/dx/disk-cache` after the Bazel verb. They skip it when
the workspace `.bazelrc`, a file it imports, `~/.bazelrc`, or the system rc
sets `--disk_cache` or `--nodisk_cache`. Under `CI=true` and inside Bazel
tests no disk cache is added. Turn it off for one run with `-- --disk_cache=`.

```sh
bazel run @rules_dx//:dx -- build //... -- --disk_cache=
```

## Hooks

`hook create`, `hook remove`, `hook stop`, and `hook guard` read one JSON
object from stdin for Claude Code hooks.

Run as exactly `dx worktree hook <kind>`, hooks ignore `dx.toml` and `DX_*`
variables and always print text. `create`, `remove`, and `stop` exit `1` on
any failure, including empty or invalid stdin.

- `create` reads `name` and `cwd`. It leases a slot on branch
  `worktree-<name>` (created from the default branch when new) and prints only
  the path. Subagent worktrees get no `remove` hook, so `create` first frees
  the session's earlier `worktree-*` slots that are at least a minute old,
  clean, and fully pushed. A slot freed this way goes to a later `create`,
  never the one that freed it. A slot that another agent leased in the
  meantime is left alone. The slot is held by `worktree-<name>` until the
  first `dx worktree add` or `release` that the same session runs inside the
  slot. That agent then becomes the holder.
- `remove` reads `worktree_path` and releases that slot. Paths outside the
  pool are ignored.
- `stop` reads `agent_id` for the `SubagentStop` event. It marks the slots
  that agent holds as held by a gone holder. Slots no agent has claimed are
  not marked; a later `create` frees them.
- `guard` reads `tool_input.command`. It exits `2` with a message on stderr
  when the command runs `git worktree add`, `remove`, or `prune`. When
  `agent_id` is set and the command runs `dx worktree`, it prints an
  `updatedInput` that exports `DX_WORKTREE_HOLDER=<agent_id>` before the
  command. When stdin is empty or not valid JSON, it prints a note on stderr
  and exits `0`. It exits `0` otherwise.

Commit `.claude/settings.json`. The hooks run the repo launcher, so `dx` does
not need to be on `PATH`:

```json
{
  "hooks": {
    "WorktreeCreate": [{"hooks": [{"type": "command", "command": "\"${CLAUDE_PROJECT_DIR:-.}\"/tools/dx worktree hook create"}]}],
    "WorktreeRemove": [{"hooks": [{"type": "command", "command": "\"${CLAUDE_PROJECT_DIR:-.}\"/tools/dx worktree hook remove"}]}],
    "SubagentStop": [{"hooks": [{"type": "command", "command": "\"${CLAUDE_PROJECT_DIR:-.}\"/tools/dx worktree hook stop"}]}],
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
`CODEX_THREAD_ID`), the agent (`DX_WORKTREE_HOLDER`, set by `hook guard`), and
the nearest ancestor of `dx` that is not a shell. Two agents in one session are
different holders, and `list` shows each one. A slot from `hook create` goes
to the first agent of the same session that runs `dx worktree add` or
`release` inside it. A holder is gone when its process
exits or `hook stop` marks it. Any other `dx` command run inside a slot leased
to a different session exits `2` and names the holder. `dx status`
inside a slot shows the lease. `worktree`, `version`, and `completion` always
run.

## Examples

```sh
bazel run @rules_dx//:dx -- worktree add feature-x
bazel run @rules_dx//:dx -- worktree add feature-x --create=origin/main
bazel run @rules_dx//:dx -- worktree add feature-x --output=path
bazel run @rules_dx//:dx -- worktree release
bazel run @rules_dx//:dx -- worktree list --output=json
bazel run @rules_dx//:dx -- worktree prune
bazel run @rules_dx//:dx -- worktree prune --apply
```
