# `dx worktree`

```text
dx worktree add <branch> [--create=<ref>] [--force] [--output=path]
dx worktree release [<path>] [--force]
dx worktree list [--disk]
dx worktree path <branch>
dx worktree prune [--apply] [--bases]
dx worktree hook create|remove|stop|guard
```

Keeps a small pool of warm checkouts, called slots, so a new worktree reuses
a slot that already has a Bazel server, output base and caches. Slots live at
`<cache>/dx/worktrees/<repo>/<n>`. `<cache>` is `$XDG_CACHE_HOME`, else
`$HOME/.cache`, else `%LOCALAPPDATA%`. Lease records live in
`<cache>/dx/worktrees/<repo>/.dx/`. `<repo>` is the basename of the main
checkout, or `<repo>-2`, `<repo>-3` when another repository already uses that
name.

A bare `dx worktree` runs `list`.

Flags: `--create=<ref>` (add only), `--force` (add and release), `--disk`
(list only), `--apply` (prune only). A flag on the wrong subcommand exits `2`.
Scopes: none. `dx worktree` takes a subcommand and its arguments.
Reports: none.

`--output text|json|path`. `path` works with `add` only. Text prints the lines
above. `add` prints `dx worktree: <branch> is in slot <n> at <path>`. When
`add` starts a new branch, it first prints
`dx worktree: started <branch> from <base>`, on stderr with `--output=path`.
`<base>` is the `--create` ref, else `origin/HEAD`, `origin/main`, or
`origin/master`.
`release` prints `dx worktree: released slot <n> at <path>`, or
`dx worktree: removed <path>` for an external worktree. Releasing a slot that
is already free prints `dx worktree: slot <n> is already free` and exits `0`.
JSON reports it as a `slot` event with `slot`, action `release`, and state
`free`.

`--output=json` streams `command_started`, one `slot` event per slot, and
`command_finished`. `list` adds a `summary` event after the slots. `prune`
emits `slot` events with action `prune`, action `prune_base` for output
bases, action `prune_cache` for rebuildable caches in free slots, and action `prune_downloads` for repository cache downloads.

When `git fetch origin` fails, `add` still uses local refs. Text prints
`dx worktree: cannot fetch origin, using local refs: <error>` before the slot
line. JSON emits a `notice` event with level `warning` and code `fetch_failed`
after `command_started`. `hook create` prints the same line on stderr.
When the caller already holds a slot on `<branch>`, `add` returns that slot
without fetching origin.

`add` also warns when the holder pid it records is not running (code
`holder_gone`): the slot can then be reclaimed once its tree is clean and
pushed, so run `add` from the long-lived agent shell. When the same session and
agent re-runs `add` on a branch it holds and the recorded pid is dead, `add`
re-pins the lease to the live caller and warns with code `holder_renewed`.

When the pool's disk, the disk of a Bazel output root, or the disk of the
[nested-Bazel root](#nested-bazel-tests) has under 20 GiB and
under 10% free, or under 1 GiB free whatever its size, `add` and `hook create`
(after they fetch and pick a slot) and `release` drop orphaned output bases,
then repository cache downloads unused for 30 days, then the test cases of
free nested-Bazel slots, then free slots' output bases, each oldest first, and print one of these lines before
the slot line. `add` and `hook create` keep the output base of the slot they
lease, so it stays warm for the build that follows.

- `dx worktree: low disk at <path>: <free> free; removed <what>; <free> free now`
- `dx worktree: low disk at <path>: <free> free; nothing to reclaim`
- `dx worktree: low disk, cannot reclaim output bases: <error>`

The first two end with `; run dx worktree prune --apply` while disk stays low,
so a pool with nothing to reclaim prints
`low disk at <path>: <free> free; nothing to reclaim; run dx worktree prune --apply`.
All three use the `disk_low` notice code. JSON emits a `notice` event with
level `warning` and code `disk_low` before the `slot` event.
Reclaim stops the idle Bazel server of a free slot before it deletes that
slot's output base; a leased slot, one with a build running, and one whose
server will not stop are never touched.

When the pool's disk still has under 1 GiB free after that reclaim and the
slot they picked is a free slot, `add` and `hook create` stop that slot's idle
Bazel server and drop its output bases as a last resort. The `disk_low` line
then ends with
`disk full: <free> free; removed the output bases of slot <n>, which this lease reuses; <free> free now`.
When the disk is still that full, they refuse before they check anything out, with code `disk_full`,
exit `1`, and the message
``disk full: <free> free at <path>; cannot check out <branch>; free disk space with `dx worktree prune --apply --bases` ``.
A refusal emits no `slot` event. A branch you already hold still resolves to
its slot.

`slot` fields: `action` (`add`, `release`, `list`, `path`, `prune`,
`prune_base`, `prune_cache`, `prune_downloads`), `path`, `state`, `server`, and optional
`slot`, `branch`, `holder`, `reason`, `base`, `disk_bytes`, and `entries`.
`busy` is also set on `list` rows: `true` when a dx command is running in
the slot, even if the slot is free.
`base` is set on `add` when it starts a new branch. `disk_bytes` is set for
`prune`, `prune_base`, `prune_cache`, and `prune_downloads` events other than `kept` and
`skipped`, and for `list` with `--disk`. `entries` is set for
`prune_downloads`: the number of downloads, and `path` is the repository
cache's `content_addressable` directory.

`state` values:

- `held`: `add`, `path`, and `list` for a leased slot.
- `free`: `release` and `prune` of a slot, and `list` for an unleased slot.
- `reclaimable`: `list` for a held slot that `add` may take over.
- `external`: `list` and `prune` for a worktree outside the pool.
- `removed`: `release` of an external worktree.
- `free`: also `prune_base` for the output base of a free slot, and
  `prune_cache` for a cache in a free slot, with its `slot`.
- `kept`: `prune` for an external worktree it keeps, and `prune_base` for an
  orphaned or nested-Bazel output base it keeps, with its `reason`.
- `orphaned`: `prune_base` for an output base whose workspace is gone.
- `nested`: `prune_base` for the output base of a test case in a free
  nested-Bazel slot.
- `skipped`: `prune` for a slot, `prune_cache` for a cache of a free slot, and `prune_base` for an output base (with its
  `slot` when it belongs to a free slot), that `prune --apply` could not
  delete, with a `reason` (`server`, `held`, or `busy`). This includes a
  nested-Bazel test's output base. `prune` events in this state have no
  `disk_bytes`.
- `stale`: `prune_downloads` for repository cache downloads unused for 30
  days.

`summary` fields: `slots`, `servers`, `servers_rss_bytes`, and
`disk_cache_bytes`, plus `disk_bytes` with `--disk`.

Exit codes: `0` success, `2` usage errors (unknown subcommand or hook name,
wrong arguments, an invalid branch name (`invalid_branch`), a path that is not
a slot (`not_a_slot`, including a bare `release` from the main checkout),
a flag on the wrong subcommand, an unknown `--create` ref,
`--create` on an existing branch, `release` of a slot another holder holds
without `--force`, any other `dx` command in a slot leased to another
session or in a slot that `add` is switching, and running outside a git repository, in a bare repository, or where
the main checkout cannot be found), `1` operational failures (a branch already
checked out elsewhere or held by another holder without `--force`, a dirty or
unpushed slot or external worktree on `release` without `--force`, `slot_busy`
when a `dx` command runs in the slot that `add --force` or `release` wants, even
with `--force`, a busy pool lock, `no_home` when no cache directory can be found,
`lease_changed` when another holder took the slot during the command, `path` finds
no checkout, `prune` finds work without `--apply`, no default branch to start
a new branch from, and any `hook create`, `hook remove`, or `hook stop`
failure), and `128` plus the signal number when Ctrl-C, `SIGTERM`, or `SIGHUP`
stops `list --disk` or `prune` (for example `143` after `SIGTERM`), with
stderr `dx: cancelled: stopped by signal N`.

## Subcommands

`add <branch>` leases a slot, checks out `<branch>`, and prints where it is.
It picks the slot that last held the branch, then a slot with the same
`MODULE.bazel.lock`, then the most recently used, whatever the free disk.
When disk is low, under 20 GiB and under 10% free or under 1 GiB free, it then
deletes orphaned output bases, then repository cache downloads unused for 30
days, then the test cases of free nested-Bazel slots, then the output bases of
the other free slots,
least recently used first, until each output base's disk is no longer low. It
keeps the output base of the slot it picked. It reuses a free slot before it creates one. Files
matched by `.worktreeinclude` in the main checkout are copied in. `--create=<ref>`
starts a new branch from `<ref>`. Without it, `add` uses the local `<branch>`,
then `origin/<branch>`, and otherwise starts `<branch>` from the default branch
(`origin/HEAD`, else `origin/main`, else `origin/master`). `--output=path` prints
only the path on stdout and writes the `dx worktree: ...` warning lines to
stderr. Submodule checkouts and `--separate-git-dir` clones work too.

`add` refuses when the branch is already checked out in a slot held by another
holder or in a worktree outside the pool, and when `--create` names a branch
that exists. `add` returns your own slot for
`<branch>` as is. `add --force` hands a slot that holds `<branch>` for another
holder to you as is, without resetting it.
`add` takes over a held slot, including one that holds `<branch>`, only when
its holder is gone, no `dx` command runs in it, the tree is clean, and no
commit is unpushed. It does not wait. Otherwise it refuses and says which
condition fails. `add` never hands out, resets, or deletes a slot while a `dx`
command runs in it, free or held.
`add` also deletes free slots idle for 30 days.

`release [<path>]` frees the slot at `<path>`, or the slot you are in. A
relative `<path>` is read from the directory you ran dx in, including under
`bazel run`. It
stops the slot's Bazel server and detaches the checkout. The branch and its
commits stay in the repository. It refuses a dirty tree or unpushed commits
unless you pass `--force`; uncommitted changes are then wiped by the next `add`
of that slot. Only the holder can release a held slot, unless it is
reclaimable or you pass `--force`. Another agent in the same session is
another holder. While a `dx` command runs in the slot, `release` exits `1` with
`slot_busy`, even with `--force`. When disk is low, `release` deletes orphaned
output bases, then the test cases of free nested-Bazel slots, then the output
bases of free slots, least recently used first,
until each output base's disk is no longer low.

`release <path>` on an `external` worktree of this repository, such as an old
`.claude/worktrees/<task>` checkout, stops its Bazel server, removes the
checkout and its output base, and keeps the branch. It refuses a dirty tree or
unpushed commits unless you pass `--force`, which removes it anyway. The main
checkout is never removed.

Each refusal of a subcommand ends with the next step, such as `pass --create=<ref>`,
`run the command again`, `pass a path from dx worktree list`, or
`run dx worktree add <branch>`.

`list` prints one line per slot and a totals line:

```text
1	feature-x	session 7f3a agent a91c pid 4121	held	server	-	/home/me/.cache/dx/worktrees/app/1
2	(detached)	-	free	no server	-	/home/me/.cache/dx/worktrees/app/2
3	review-y	pid 4180 (gone)	held	no server	-	/home/me/.cache/dx/worktrees/app/3
1 bazel server(s) running, 2.1 GiB resident
```

A holder that has exited shows ` (gone)` after its name.

The columns are slot, branch, holder, state, server, disk use, and path. The
disk column shows `-` unless you pass `--disk`, which measures each slot and
its output base and adds the total to the last line, followed by the size of
the shared disk cache (`0 B` when it does not exist). It is slow on a large
pool. The disk cache is the one Bazel uses for `build`: the `--disk_cache` an
rc file sets (lines in `--config` groups do not count), otherwise
`<cache>/dx/disk-cache`. It is `0 B` when the cache is turned off, and under
`CI=true`.

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
pushed)`. A slot where a dx command is running has ` busy` appended to its
state, such as `free busy`, and `busy: true` in the JSON event.

`path <branch>` prints the path of the checkout that holds `<branch>`, and
fails with exit `1` when none does.

`prune` lists free slots idle for 30 days, removable `external` worktrees, and
Bazel output bases of removed worktrees, including checkouts removed from
`.claude/worktrees`. An external worktree is removable when it is clean, fully
pushed, not locked, and without a running Bazel server, and either is on a
branch that tracks `origin/<branch>` and is merged into the default branch or
whose `origin/<branch>` is deleted, or is detached at a commit in the default
branch and git has not touched it for 24 hours. It also lists the output
bases of the other free slots whose disk is low, least recently used first, as
`output base of free slot <n>, <size>: <dir>`. `--bases` lists the output bases of every free slot, whatever the disk state, least recently used first, except the slots already listed as idle. `--bases` works with `prune` only.
The same trigger (`--bases`, or low disk) also lists rebuildable caches in free slots: any direct child directory of the slot's `.dx` that holds a standard `CACHEDIR.TAG` (see <https://bford.info/cachedir/>), as `cache of free slot <n>, <size>: <dir>`. `--apply` deletes them unless the slot is leased or a dx command runs in it (`skipped cache of free slot <n> (<reason>)`), and the owning tool recreates them. Symlinks are never followed. It also lists the downloads in
Bazel's repository cache that no build used for 30 days. Only the default
cache, `cache/repos/v1/content_addressable` under each Bazel output root, is
checked. A cache moved with `--repository_cache` is not cleaned, and the
`contents` directory next to it is not touched. A deleted
download is fetched again when a build needs it. `--apply` deletes the idle free slots and stops each slot's
Bazel server and deletes those output bases; the other slots stay. `--apply` removes the
external worktrees with their output bases and keeps the branches. It deletes nothing without `--apply`. It exits `1` when something
would be pruned and `--apply` is absent, and `0` when there is nothing to prune.

Without `--apply`, text prints one line per item, and a closing line
`dx worktree: <size> to reclaim; run dx worktree prune --apply`. The item
lines are `slot <n> idle <days> days, <size>: <path>`,
`external worktree <branch>, <size>: <path>`,
`output base of free slot <n>, <size>: <dir>`,
`cache of free slot <n>, <size>: <dir>`,
`output base of removed <workspace>, <size>: <dir>`,
`output base of nested Bazel test <case>, <size>: <dir>`, and
`repository cache downloads unused for 30 days, <n> entries, <size>: <dir>`,
one per cache. With `--apply` the closing
line is `dx worktree: pruned <size>`, and `dx worktree: nothing to prune`
replaces everything when nothing qualifies.

Every other external worktree is kept and printed as
`kept external worktree <branch> (<reason>): <path>`, with `(detached)` for
`<branch>` when no branch is checked out. The reason is `locked`, `server`
(a Bazel server runs in it), `dirty`, `unpushed`, `local` (the branch does not
track `origin/<branch>`), `unmerged` (not in the default branch, or there is no
default branch), or `recent` (detached, and git touched it in the last 24
hours). Kept lines print before the other lines and before
`dx worktree: nothing to prune`, and do not change the exit code. JSON reports
each as `prune` with state `kept` and that `reason`.

`--apply` finds and deletes everything under the pool lock. It checks each
removable external worktree again right before it deletes it. One that became
locked, got a Bazel server, uncommitted changes, or unpushed commits is kept
the same way, as is one whose delete failed and that is still partly on disk,
with reason `busy`. It still exits `0`.

An output base whose workspace is gone but that a Bazel server still uses is
kept and printed as `kept output base of removed <workspace> (server): <dir>`.
JSON reports it as `prune_base` with state `kept` and reason `server`.

A free slot's output base whose Bazel server will not stop, whose slot was
taken, or whose slot runs a `dx` command, is kept and printed as
`skipped output base of free slot <n> (<reason>): <dir>`, with reason `server`,
`held`, or `busy`. An orphaned output base that cannot be deleted is printed as
`skipped output base of removed <workspace> (busy): <dir>`; this also covers
the output base of a slot that `prune` deleted while the base stayed. A slot
that was taken after the plan or is in use is printed as
`skipped slot <n> (<reason>): <path>`. So is a slot whose checkout is still
partly on disk after the delete; `dx worktree add` no longer leases it, and the
next prune or `add` tries to delete it again. Skipped items are not counted in
`pruned <size>`, and the command still exits `0`.

`--apply` checks each download again right before it deletes it. One that a
build used since the check is kept without a line and is not counted in
`pruned <size>`.

### Nested-Bazel tests

Tests that run Bazel inside Bazel keep their servers under the nested-Bazel
root: `DX_NESTED_BAZEL_ROOT` when you export it, else `/tmp/dxnb-<uid>`. Each
slot there has a `lock` file the test holds while it runs, an output user root
`u`, and one workspace per test case under `w`.

`prune` treats each test case, its output base and its workspace, as one item,
printed as `output base of nested Bazel test <case>, <size>: <dir>`. It lists
the cases whose workspace has not changed for 30 days, and with `--bases` or
when the nested-Bazel root's disk is low, every case, least recently used
first. It only looks at slots whose `lock` it can take without waiting, and
never creates a `lock` file. `--apply` holds the slot's lock while it deletes.
A case whose slot a test locked after the check is printed as
`skipped output base of nested Bazel test <case> (busy): <dir>`; one whose Bazel
server still runs is kept as
`kept output base of nested Bazel test <case> (server): <dir>`.

## Shared disk cache

The main checkout and every slot share one Bazel disk cache at
`<cache>/dx/disk-cache`, so a branch built in one checkout is not rebuilt in
another. `dx build`, `test`, `coverage`, `run`, and `deploy` add
`--disk_cache=<cache>/dx/disk-cache` after the Bazel verb. They skip it when
the workspace `.bazelrc`, a file it imports, `~/.bazelrc`, or the system rc
sets `--disk_cache` or `--nodisk_cache`. Under `CI=true` and inside Bazel
tests no disk cache is added. Turn it off for one run with `-- --disk_cache=`.
`dx worktree list --disk` and the JSON summary measure the cache Bazel uses for
`build`, which is the rc value when an rc file sets one.

```sh
bazel run @rules_dx//:dx -- build //... -- --disk_cache=
```

### Disk cache size

When dx adds the disk cache, it also bounds it, using the same values as the
vendored preset:

```text
--experimental_disk_cache_gc_max_size=20G
--experimental_disk_cache_gc_max_age=14d
```

Bazel then deletes the oldest entries once the cache is over 20G, and entries
older than 14 days. dx adds both flags or neither. It leaves them out when an
rc file it reads sets either flag, or when the run's arguments set either flag
or `--disk_cache`. Your own values always win.

Bazel cleans the cache only while a server is idle. The cleanup starts after
`--experimental_disk_cache_gc_idle_delay`, which is 5 minutes by default, and
only if the server's last command carried the bound. A server that shuts down
sooner, because of a short `startup --max_idle_secs`, never cleans. So when an
rc file sets `startup --max_idle_secs` below 900, dx also adds
`--experimental_disk_cache_gc_idle_delay=<max_idle_secs / 3>s`. It does this
even when you set your own cache and bound, and skips it when an rc file or
the run's arguments set the idle delay.

Change the bound for every run in `~/.bazelrc`:

```text
common --experimental_disk_cache_gc_max_size=40G
common --experimental_disk_cache_gc_max_age=30d
```

Or for one run:

```sh
bazel run @rules_dx//:dx -- build //... -- --experimental_disk_cache_gc_max_size=40G
```

## Hooks

`hook create`, `hook remove`, `hook stop`, and `hook guard` read one JSON
object from stdin: the hook input of Claude Code, Codex, Cursor, or Gemini CLI.

Run as exactly `dx worktree hook <kind>`, hooks ignore `dx.toml` and `DX_*`
variables and always print text. `create`, `remove`, and `stop` exit `1` on
any failure, including empty or invalid stdin. When `create`, `remove`, or
`stop` gets input without its field, the message names the field and the hook
event that runs it.

- `create` reads `name` and `cwd`. Without `cwd`, it uses the current
  directory. It leases a slot on branch `worktree-<name>`, where characters
  other than letters, digits, `-`, `_`, and `.` become `-`, so `my task`
  gives `worktree-my-task`. It prints only the
  path on stdout. Warnings such as
  `dx worktree: cannot fetch origin, using local refs: <error>` go to stderr
  and do not change the exit code. It reuses a local branch with
  that name. A new branch starts from the default branch. If only `origin` has
  `worktree-<name>`, it uses `worktree-<name>-2` (or the next free number).
  Subagent worktrees get no `remove` hook, so `create` first frees
  the session's earlier `worktree-*` slots whose holder is gone, and that are
  clean, fully pushed, and not running a `dx` command. A slot whose holder
  still runs is kept, however old. A slot freed this way goes to a later `create`,
  never the one that freed it. A slot that another agent leased in the
  meantime is left alone. The slot is held by `worktree-<name>` until the
  first `dx worktree add` or `release` that the same session runs inside the
  slot. That agent then becomes the holder.
- `remove` reads `worktree_path` and releases that slot. Paths outside the
  pool are ignored. Release warnings, such as a low-disk reclaim, go to stderr
  as `dx worktree: <message>` and do not change the exit code.
- `stop` reads `agent_id` for the `SubagentStop` event. It marks the slots
  that agent holds as held by a gone holder. Slots no agent has claimed are
  not marked; they stay held until the session ends. Cursor's `subagentStop` input has
  no `agent_id`; `stop` does nothing for it and exits `0`.
- `guard` reads `tool_input.command`. It exits `2` with a message on stderr
  when the command runs `git worktree add`, `remove`, or `prune`. It tells
  the agent from the input: `hook_event_name` `BeforeTool` is Gemini CLI,
  `cursor_version` or `conversation_id` is Cursor, and anything else has the
  Claude Code shape that Codex also sends. For Cursor and Gemini CLI, whose
  shells carry no session id, a command that runs `dx` gets
  `export DX_WORKTREE_SESSION=<id>` before it, where the id is `session_id`
  or else `conversation_id`. When `agent_id` is set and the command runs
  any `dx` command, it also exports `DX_WORKTREE_HOLDER=<agent_id>`. Codex
  sends no `agent_id`; when `CODEX_THREAD_ID` is set and differs from the
  payload `session_id`, that `session_id` (the subagent's own thread) is
  exported as the holder instead. The
  rewritten command goes in `hookSpecificOutput.updatedInput`, or
  `hookSpecificOutput.tool_input` for Gemini CLI, and keeps the other
  `tool_input` fields. The guard never approves a command for the agent.
  When stdin is empty or not valid JSON, it prints a note on stderr and exits
  `0`. It exits `0` otherwise.

Commit `.claude/settings.json`. The hooks run the repo launcher, so `dx` does
not need to be on `PATH`:

```json
{
  "hooks": {
    "WorktreeCreate": [{"hooks": [{"type": "command", "command": "\"${CLAUDE_PROJECT_DIR:-.}\"/tools/dx worktree hook create"}]}],
    "WorktreeRemove": [{"hooks": [{"type": "command", "command": "\"${CLAUDE_PROJECT_DIR:-.}\"/tools/dx worktree hook remove"}]}],
    "SubagentStop": [{"hooks": [{"type": "command", "command": "\"${CLAUDE_PROJECT_DIR:-.}\"/tools/dx worktree hook stop"}]}],
    "PreToolUse": [{"matcher": "Bash", "hooks": [{"type": "command", "command": "\"${CLAUDE_PROJECT_DIR:-.}\"/tools/dx worktree hook guard"}]}]
  },
  "permissions": {
    "allow": [
      "Bash(tools/dx check *)",
      "Bash(tools/dx test *)",
      "Bash(tools/dx build *)",
      "Bash(tools/dx list-tests *)",
      "Bash(tools/dx status *)",
      "Bash(tools/dx version *)"
    ]
  }
}
```

Cursor runs these hooks too while its Third-Party Imports setting is on, which
is the default. Codex and Gemini CLI get only the guard, in files of the same
shape: `.codex/hooks.json` with event `PreToolUse` and matcher `Bash`, and
`.gemini/settings.json` with event `BeforeTool` and matcher
`run_shell_command`. Codex runs project hooks only after you trust them with
`/hooks`; Gemini CLI runs them only in a trusted folder.

| Agent | Hooks file | Session |
| --- | --- | --- |
| Claude Code | `.claude/settings.json` | `CLAUDE_CODE_SESSION_ID` |
| Codex | `.codex/hooks.json` | `CODEX_THREAD_ID` |
| Cursor | `.claude/settings.json` | `DX_WORKTREE_SESSION`, set by `hook guard` |
| Gemini CLI | `.gemini/settings.json` | `DX_WORKTREE_SESSION`, set by `hook guard` |

`dx init --apply --with=agents` writes each file when it is absent and never overwrites it. If
one exists, it prints the hook entries that file lacks, and the missing
`permissions.allow` entries of `.claude/settings.json`. It also writes
the launcher and the agent commands block in `AGENTS.md`; see
[`dx init`](init.md). `dx status` reports the `agents` check as `warn` when a
hook is missing from `.claude/settings.json`, or from `.codex/hooks.json` or
`.gemini/settings.json` when they exist, or `CLAUDE.md` does not import
`AGENTS.md`.

## Leases

The holder is the agent session (`CLAUDE_CODE_SESSION_ID`, `CODEX_THREAD_ID`,
or `DX_WORKTREE_SESSION`, in that order; see the table above), the agent (`DX_WORKTREE_HOLDER`, set by `hook guard`), and
a process: `CLAUDE_PID` when it is an ancestor of `dx`, else the nearest
ancestor that is not a shell or a wrapper such as `timeout`, `gtimeout`,
`flock`, `env`, `nice`, `nohup`, `setsid`, `stdbuf`, `time`, or `xargs`. Two agents in one session are
different holders, and `list` shows each one. Under Cursor and Gemini CLI, only commands that
`hook guard` sees run with the session, so `dx` started from a script or `make`
has none and is refused in a slot that a session holds. A slot from `hook create` goes
to the first agent of the same session that runs `dx worktree add` or
`release` inside it. A holder is gone when its process
exits or `hook stop` marks it. `hook stop` fires at the end of every turn, so it marks a
holder as stopped, not finished (#3044): a gone holder keeps its slot until
30 minutes after its last activity (the stop, or the lease), and only then can
another agent reclaim it. A slot no agent has claimed stays held until its
session ends. Any other `dx` command run inside a slot leased
to a different session exits `2` and names the holder. It also exits `2` when
another agent of the same session holds the slot. A caller with no agent, a
slot from `hook create` that no agent has claimed, and a holder that is gone
keep the session rule. `dx status`
inside a slot shows the lease. Every other `dx` command marks the slot busy
while it runs, so no `add`, `release`, or `hook create` resets or frees it.
While `add` switches a slot, such a command waits, and exits `2` with `slot N is
being switched by dx worktree; run the command again` if the switch takes
longer than two minutes. `worktree`, `version`, and `completion` always run.

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
