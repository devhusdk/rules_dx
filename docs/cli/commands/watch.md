# `dx watch`

Use `dx watch` to rerun a build, test, run or check command each time you save a file.

```sh
dx watch build //app/...
dx watch test //cli/...
dx watch test --affected
dx watch lint --output json --affected
dx watch run //app:bin
```

## Usage

```text
dx watch [--apply] [--debug|--release] [flags] <build|test|run|lint|typecheck|format|check> [scope ...] [-- bazel-options|app-args ...]
```

Flags go to the wrapped command and are checked as `dx <command>` checks them. A flag the wrapped command rejects fails before watching starts. Args after `--` reach the wrapped command unchanged.

## Flags

| Flag | What it does |
| --- | --- |
| `--apply` | Write on every iteration. Bare `lint`, `typecheck`, `format`, and `check` only report. |
| `--debug`, `--release` | Pick the build mode. |
| `--here` | Resolve the scope once from the current directory. |
| `--affected` | Re-resolve the affected targets each iteration. |
| `--since <ref>` | With `--affected`: diff from this ref instead of the merge-base. |
| `--base <ref>` | With `--affected`: take the merge-base with this branch instead of the default branch. |
| `--poll` | Poll the tree every second instead of using the platform watcher, on any platform. |
| `--fail-on`, `--report`, `--diff`, `--unsafe`, `--include-edits`, `--max-findings` | Passed to the wrapped command when it takes them. |
| `-o, --output text\|json` | Choose the output format. |
| `--workspace <dir>` | Use this workspace directory. |
| `-q, --quiet` | Hide summaries. |
| `-v, --verbose` | Show more logs. |
| `--color auto\|always\|never` | Set color output. |
| `--build-slots <auto\|n\|off>` | Set build slots: `auto`, a count, or `off`. |
| `--build-slots-wait <duration>` | Give up waiting for a build slot after this long. `0` fails at once. |
| `--bazel-startup-option <token>` | Pass one Bazel startup option. Repeatable. |

## Scopes

The wrapped command and its scopes, resolved again on each iteration. Only `build`, `test`, `run`, `lint`, `typecheck`, `format` and `check` can be watched. Other commands fail with `not watchable`.

`dx watch` runs on your machine only. It refuses to start when `CI=true`.

## Behavior

- dx ignores changes under `.dx/local/`, `.dx/bazelrc/`, `.git/`, and `bazel-*` in the workspace, and the wrapped command's `--report` files. Edits to `BUILD` and `MODULE.bazel` files rerun the command.
- Changes made during a run queue one more iteration, except under `run`. Iterations never overlap. A failing command does not stop the watch.
- `watch run` restarts the app on each change. The old app and its children get `SIGTERM`, then are killed after 10 seconds. The app gets no stdin. An app that exits on its own is relaunched on the next change.
- Each iteration takes a build slot. Watch holds none while it waits.
- If nothing arrives for 30 s but `git status` shows a change, watch prints one line suggesting `--poll`. Network mounts and containers often miss inotify events.

## Output

Each iteration writes the wrapped command's output. Watch adds a header and a pass/fail line per iteration, one line per trigger, and a separator before each rerun. On a terminal it also clears the screen before a rerun. `--quiet` drops them. With `--output json`, `--diff`, or `--report <format>=-`, they go to stderr.

```text
watch:test:debounce=200ms scope=//... iteration=1
iteration 1: ok in 4.2s; watching (Ctrl-C to stop)
changed /work/repo/src/main.rs (+1 more)
----------------------------------------
watch:test:debounce=200ms scope=//... iteration=2
iteration 2: failed (exit 1) in 1.3s; watching
stopped
```

`dx watch` writes no reports of its own. The wrapped command writes the reports you pass with `--report`.

Under JSON, each iteration is one event stream ending in `command_finished`. A restarted app reports `143`.

## Exit codes

Watch runs until stopped and has no success exit.

| Code | Meaning |
| --- | --- |
| `3` | The watcher could not start or lost the workspace (`watch_failed`). |
| `2` | Usage error, `not watchable`, or `CI=true`. |
| `129` | `SIGHUP`. |
| `130` | Ctrl-C. |
| `141` | stdout closed. |
| `143` | `SIGTERM`. |
