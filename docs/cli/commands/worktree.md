# `dx worktree`

Use `dx worktree` to give each AI agent session its own warm checkout, called a slot.
A slot keeps its Bazel server, its output base and its caches between branches. The
Bazel server is the background process that keeps build state in memory. The output
base is the directory where Bazel keeps build outputs.

```sh
tools/dx worktree add feature-x
tools/dx worktree add feature-x --create=origin/main
tools/dx worktree add feature-x --path-only
tools/dx worktree release
tools/dx worktree list --disk
tools/dx worktree prune --apply
```

## Usage

```text
dx worktree add <branch> [--create <ref>] [--force] [--path-only]
dx worktree release [<path>|<slot-number>] [--force]
dx worktree list [--disk]
dx worktree path <branch>
dx worktree prune [--apply] [--bases]
dx worktree hook create|remove|stop|guard|bazel [-- <bazel-args>]
```

A bare `dx worktree` runs `list`. Slots live in `<cache>/dx/worktrees/<repo>/<n>`.
`<cache>` is `$XDG_CACHE_HOME`, else `$HOME/.cache`, else `%LOCALAPPDATA%`.

## Flags

| Flag | What it does |
| --- | --- |
| `--create <ref>` | Start a new branch from `<ref>`. `add` only. |
| `--force` | `add`: take a slot held by another holder. `release`: discard unpushed work or free another holder's slot. |
| `--path-only` | Print only the path on stdout. `add` only. |
| `--disk` | Measure disk use per slot and mark surplus slots `prunable`. `list` only, slow. |
| `--apply` | Delete what `prune` lists. `prune` only. |
| `--bases` | List every free slot's output base. `prune` only. |
| `-o, --output text\|json` | Choose the output format. |
| `--workspace <dir>` | Use this workspace directory. |
| `-q, --quiet` | Hide summaries. |
| `-v, --verbose` | Show more logs. |
| `--color auto\|always\|never` | Set color output. |
| `--build-slots <auto\|n\|off>` | Set build slots: `auto`, a count, or `off`. |
| `--build-slots-wait <duration>` | Give up waiting for a build slot after this long. `0` fails at once. |
| `--bazel-startup-option <token>` | Pass one Bazel startup option. Repeatable. |

A flag on the wrong subcommand exits `2`.

## Scopes

None. `dx worktree` takes a subcommand and its arguments. It writes no reports.

## Subcommands

`add <branch>` leases a slot and checks out `<branch>`. It prints where the
slot is. Files matched by `.worktreeinclude` in the main checkout are copied
in; the copy of `user.bazelrc` leaves out `startup` lines that set
`--output_base`. Without `--create`, it uses the local `<branch>`, then `origin/<branch>`,
then starts `<branch>` from `origin/HEAD`, `origin/main` or `origin/master`.
It refuses a branch checked out by another holder or outside the pool. It
returns your own slot for `<branch>` as is. It takes over a held slot only
when its holder is gone, the tree is clean, nothing is unpushed and no `dx`
command runs in it and no process other than `git` has it as its working
directory. If `git fetch origin` fails, it uses local refs and warns.

`release [<path>|<slot-number>]` frees the slot at `<path>`, the slot numbered
`<slot-number>` when the argument is only digits and no such path exists, or the
slot you are in. It stops the slot's Bazel server and detaches the checkout.
The branch stays. It refuses a dirty tree or unpushed commits without `--force`.
Another holder's slot is freed only when it is reclaimable or with `--force`. It exits `1` with
`slot_busy` while a `dx` command runs in the slot. On an external worktree of
this repository it removes the checkout and keeps the branch.

Commands that run Bazel inside a slot start its server with
`--max_idle_secs=300`, so an idle slot's server exits after five minutes.
`--build-slots <n>` limits how many run at once and `DX_BUILD_SLOT_MEMORY` sets
the memory each is planned to use.

The working-directory check reads `/proc` on Linux only. It does not see other
users' processes, processes hidden by `hidepid` or processes in other PID
namespaces, and each check of a held slot scans every process. Any process whose
working directory is in the slot, such as an idle shell or a Bazel server,
keeps the slot held and blocks a release by another holder until it exits or
`--force` is passed.

`list` prints one line per slot, then a totals line:

```text
1	feature-x	session 7f3a agent a91c pid 4121	held	server	-	/home/me/.cache/dx/worktrees/app/1
2	(detached)	-	free	no server	-	/home/me/.cache/dx/worktrees/app/2
1 bazel server(s) running, 2.1 GiB resident
```

The columns are slot, branch, holder, state, server, disk use and path. State
is `held`, `free`, `reclaimable` (the holder is gone) or `external` (outside
the pool). ` busy` is appended while a `dx` command runs in the slot. ` prunable`
is appended to a slot `add` would delete. Every `list` marks free slots idle
for 30 days. `--disk` also marks the surplus slots described below, fills the
disk column and adds the shared disk cache size to the last line.

After `add` switches its slot it deletes free slots, oldest first, while the pool
holds more than `DX_WORKTREE_MAX_SLOTS` slots (default `24`; `0` turns it off),
and one when the disk is low. A surplus slot is deleted only when it is free, no
process or `dx` command runs in it, no rebase, merge, cherry-pick, revert or
bisect is in progress, `git status --ignored` shows nothing but `bazel-*` links
and every commit is on a remote. `add` checks the oldest few slots and checks
each again under the pool lock before it deletes it.

`path <branch>` prints the path of the checkout that holds `<branch>`. It exits
`1` when none does.

`prune` lists free slots idle for 30 days, removable external worktrees, output
bases of removed worktrees, rebuildable caches in free slots, and repository
cache downloads unused for 30 days. `--bases` also lists the output base of
every free slot. Without `--apply` it deletes nothing and exits `1` when it
found something. With `--apply` it deletes the items, keeps the branches, and
exits `0`. It keeps a worktree that is locked, dirty, unpushed, unmerged, runs
a Bazel server or was touched in the last 24 hours, and prints
`kept external worktree <branch> (<reason>): <path>`.

## Output

Text prints these lines:

- `add`: `dx worktree: <branch> is in slot <n> at <path>`, after
  `dx worktree: started <branch> from <base>` for a new branch.
- `release`: `dx worktree: released slot <n> at <path>`, or
  `dx worktree: removed <path>` for an external worktree. A free slot prints
  `dx worktree: slot <n> is already free` and exits `0`.
- `prune`: one line per item, then `dx worktree: <size> to reclaim; run dx worktree prune --apply`.
  With `--apply`: `dx worktree: pruned <size>` or `dx worktree: nothing to prune`.

When the disk is low, `add`, `release` and `hook create` drop orphaned output
bases, old downloads and free slots' output bases, then
print `dx worktree: low disk at <path>: <free> free; removed <what>; <free> free now`.
If the disk stays full, `add` refuses with `disk_full`. Run
`dx worktree prune --apply --bases`.

The first `add` that turns on the main-checkout rule also prints
`dx worktree: the main checkout <path> is now protected: ...`.

`--output=json` streams `command_started`, one `slot` event per slot and
`command_finished`. `list` adds a `summary` event. A fetch failure adds a
`notice` event with code `fetch_failed`. Turning on the main-checkout rule adds
an `info` notice.

`slot` fields: `action` (`add`, `release`, `list`, `path`, `prune`,
`prune_base`, `prune_cache`, `prune_downloads`), `path`, `state`, `server`, and
optional `slot`, `branch`, `holder`, `reason`, `base`, `disk_bytes`, `entries`,
`busy` and `prunable`. `state` is `held`, `free`, `reclaimable`, `external`, `removed`,
`kept`, `orphaned`, `skipped` or `stale`. `summary` fields: `slots`,
`servers`, `servers_rss_bytes`, `disk_cache_bytes` and, with `--disk`,
`disk_bytes`.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | Operational failure: branch checked out elsewhere, dirty or unpushed slot on `release`, `slot_busy`, `lease_changed`, `no_home`, `disk_full`, `path` finds no checkout, `prune` found work without `--apply`, or a `hook` failure. |
| `2` | Usage error: unknown subcommand or hook, wrong arguments, `invalid_branch`, `not_a_slot`, a flag on the wrong subcommand, an unknown `--create` ref, `release` of another holder's slot without `--force`, any other `dx` command in a slot leased to another session, or a command the main-checkout rule refuses. |
| `128` plus signal | Ctrl-C, `SIGTERM` or `SIGHUP` stopped `list --disk` or `prune`. |

## Shared disk cache

The main checkout and every slot share one Bazel disk cache at
`<cache>/dx/disk-cache`. `dx build`, `test`, `coverage`, `run` and `deploy` add
`--disk_cache` unless an rc file sets `--disk_cache` or `--nodisk_cache`. Under
`CI=true` and inside Bazel tests no disk cache is added. Turn it off for one run:

```sh
tools/dx build //... -- --disk_cache=
```

dx also adds `--experimental_disk_cache_gc_max_size=20G` and
`--experimental_disk_cache_gc_max_age=14d` unless an rc file or your arguments
set either flag. Change them in `~/.bazelrc`:

```text
common --experimental_disk_cache_gc_max_size=40G
common --experimental_disk_cache_gc_max_age=30d
```

## Hooks

Hooks read one JSON object from stdin: the hook input of Claude Code, Codex,
Cursor or Gemini CLI. They ignore `DX_*` variables and print text.

| Hook | Reads | What it does |
| --- | --- | --- |
| `create` | `name`, `cwd` | Leases a slot on branch `worktree-<name>` and prints the path. |
| `remove` | `worktree_path` | Releases that slot. Paths outside the pool are ignored. |
| `stop` | `agent_id` | Marks the slots that agent holds as held by a gone holder and stops their Bazel servers. |
| `bazel` | nothing; the Bazel arguments follow `--` | Exits `2` when the main-checkout rule refuses this Bazel call, or for `bazel clean` in a slot. |
| `guard` | `tool_input.command` | Exits `2` for `git worktree add`, `remove` or `prune`. Exits `2` for `git checkout`, `switch`, `reset`, `rebase`, `fetch`, `pull`, `stash` or `commit`, `bazel`, and `dx build`, `test` or `check` in a slot another agent holds. Exits `2` for the same commands in the primary checkout when the caller holds a slot; the message names the slot. Read-only git, `gh` and `dx worktree add`, `release` and `list` stay allowed. |

`create`, `remove` and `stop` exit `1` on any failure, including empty or
invalid stdin. `guard` exits `0` on empty or invalid stdin. For Cursor and
Gemini CLI, `guard` prefixes a `dx` command with
`export DX_WORKTREE_SESSION=<id>`. It also exports `DX_WORKTREE_HOLDER=<agent_id>`
when the input has an `agent_id`.

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
      "Bash(tools/dx status *)",
      "Bash(export DX_WORKTREE_SESSION=*)",
      "Bash(export DX_WORKTREE_HOLDER=*)"
    ]
  }
}
```

Codex and Gemini CLI get only the guard: `.codex/hooks.json` with event
`PreToolUse` and matcher `Bash`, and `.gemini/settings.json` with event
`BeforeTool` and matcher `run_shell_command`. Codex runs project hooks after
you trust them with `/hooks`. Gemini CLI runs them in a trusted folder.
GitHub Copilot gets `.github/hooks/dx.json` with version 1 and event
`PreToolUse` (the Claude Code spelling, so Copilot sends the Claude payload)
and matcher `Bash`; a hook that exits 2 denies the command.

| Agent | Hooks file | Session |
| --- | --- | --- |
| Claude Code | `.claude/settings.json` | `CLAUDE_CODE_SESSION_ID` |
| Codex | `.codex/hooks.json` | `CODEX_THREAD_ID` |
| Cursor | `.claude/settings.json` | `DX_WORKTREE_SESSION`, set by `hook guard` |
| Gemini CLI | `.gemini/settings.json` | `DX_WORKTREE_SESSION`, set by `hook guard` |
| GitHub Copilot | `.github/hooks/dx.json` | none |

`dx init --apply --agents <list>` writes the file of each named agent when it
is absent and never overwrites one. If a file exists, it prints the missing
hook entries. It also writes the launcher and the agent commands block in
`AGENTS.md`; see the [fixed file set](init.md#fixed-file-set). `dx status`
reports the `agents` check as `warn` when a hook is missing or `CLAUDE.md` does
not import `AGENTS.md`.

## Main checkout

The main checkout is the clone itself, not a slot. The git config key
`dx.primary` keeps Bazel work out of it:

| `dx.primary` | Who is refused in the main checkout |
| --- | --- |
| unset | Nobody. |
| `protected` | Everyone. |
| `open` | Agent sessions only; see [Agent sessions](#agent-sessions). |

The first `dx worktree add` sets `dx.primary` to `protected` when it is unset.
To refuse only agents:

```sh
git config dx.primary open
```

A refused command exits `2` and prints
`dx: refused: no Bazel in the main checkout <path>. Work in a slot: tools/dx worktree add <branch>`.
JSON reports error code `main_checkout`. A refused command writes nothing.

Only commands that only read still run: `dx status`, `dx worktree`,
`dx completion`, `dx config` except `dx config override`, `--help` and
`--version`. Every other command is refused.

dx fails closed. Any other value of `dx.primary`, such as a typo, counts as
`protected`, and the message names the value. When git cannot read the
setting, dx refuses and prints why. `GIT_CONFIG`, `GIT_CONFIG_COUNT` and
`git -c` do not change what dx reads. Without git on `PATH`, dx reads the
nearest `.git` itself. It refuses when `.git/worktrees` exists, when
`.git/config` cannot be read, or when `.git/config` has `primary` in a `[dx]`
section.

`CI=true` does not skip the check.

`tools/dx` does not rebuild dx in a refused main checkout. It asks the copy
of dx in `.dx/local/launcher`, then the one in `bazel-bin`, and runs the one
that refuses. When neither can answer, it reads `dx.primary` with git and
refuses when it is set or unreadable. Without git, it reads `.git` as dx does.
`tools/dx.cmd` refuses a `.git/config` that has a `[dx]` section and the word
`primary` anywhere.

With `dx.primary` set, a Bazel `clean` in a slot exits `2`, through
`dx worktree hook bazel` or `dx clean --bazel`. Rerun with
`--noremote_accept_cached`, or release the slot and add a fresh one.

## Agent sessions

When `CLAUDECODE`, `CODEX_THREAD_ID`, `CODEX_SANDBOX`, `CURSOR_AGENT`, `GEMINI_CLI`,
`QWEN_CODE`, `AMP_CURRENT_THREAD_ID`, `AUGMENT_AGENT`, `OPENCODE`, `COPILOT_CLI`,
`AI_AGENT`, `AGENT` or `GOOSE_TERMINAL` is set, the default output is `json` for commands that support it,
and Bazel progress stays off stderr. `--output` and `DX_OUTPUT` still win. In CI
these variables are ignored.

## Leases

A slot belongs to one holder: the agent session, the agent and a process. Two
agents in one session are different holders. Any other `dx` command run in a
slot leased to a different session exits `2` and names the holder. A gone
holder keeps its slot for 30 minutes after its last activity. A slot from
`hook create` goes to the first agent of the same session that runs
`dx worktree add` or `release` in it. `dx status` in a slot shows the lease.

While a `dx` command runs in a slot, no `add`, `release` or `hook create`
resets or frees it. While `add` switches a slot, other `dx` commands wait, then
exit `2` after two minutes. `worktree`, `version` and `completion` always run.
