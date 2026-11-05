# `dx watch`

Reruns one command when files change.

```sh
dx watch build //app/...
dx watch test //cli/...
dx watch test --affected
dx watch lint --output json --affected
dx watch run //app:bin
```

```text
dx watch [--apply] [--debug|--release] [flags] <build|test|run|lint|typecheck|format|check> [scope ...] [-- bazel-options|app-args ...]
```

Flags go to the wrapped command and are checked as `dx <command>` checks them. A flag the wrapped command rejects fails before watching starts. Args after `--` reach the wrapped command unchanged.

| Flag | What it does |
| --- | --- |
| `--apply` | Write on every iteration. Bare `lint`, `typecheck`, `format`, and `check` only report. |
| `--debug`, `--release` | Pick the build profile. |
| `--here` | Resolve the scope once from the current directory. |
| `--affected` | Re-resolve the affected targets each iteration. |
| `--poll` | Poll the tree every second instead of using the platform watcher, on any platform. `DX_WATCH_POLL=1` makes polling the default. |
| `--fail-on`, `--baseline`, `--report`, `--diff`, `--include-edits` | Passed to the wrapped command when it takes them. |
| `-o, --output text\|json` | Choose the output format. |
| `--workspace`, `-q`, `-v`, `--color`, `--build-slots`, `--bazel-startup-option` | Global flags. |

Scopes: the wrapped command's scopes, re-resolved each iteration. Other commands fail with `not watchable`.

Local only. `CI=true` is refused.

## Behavior

- dx ignores changes under `.dx/`, `.git/`, and `bazel-*` in the workspace, and the wrapped command's `--report` files. Edits to `BUILD` and `MODULE.bazel` files rerun the command.
- Changes made during a run queue one more iteration, except under `run`. Iterations never overlap. A failing command does not stop the watch.
- `watch run` restarts the app on each change. The old app and its children get `SIGTERM`, then are killed after 10 seconds. The app gets no stdin. An app that exits on its own is relaunched on the next change.
- Each iteration takes a build slot. Watch holds none while it waits.
- If nothing arrives for 30 s but `git status` shows a change, watch prints one line suggesting `--poll`. Network mounts and containers often miss inotify events.

## Output

Each iteration writes the wrapped command's output. Watch adds one line per iteration and one per trigger. `--quiet` drops them. With `--output json`, `--diff`, or `--report <format>=-`, they go to stderr.

```text
watch:test:debounce=200ms scope=//... iteration=1
watching (Ctrl-C to stop)
changed /work/repo/src/main.rs (+1 more)
watch:test:debounce=200ms scope=//... iteration=2
stopped
```

Under JSON, each iteration is one event stream ending in `command_finished`. A restarted app reports `143`.

## Exit codes

Watch runs until stopped and has no success exit.

| Code | Meaning |
| --- | --- |
| `1` | Operational failure such as `watch_failed`. |
| `2` | Usage error, `not watchable`, or `CI=true`. |
| `129` | `SIGHUP`. |
| `130` | Ctrl-C. |
| `141` | stdout closed. |
| `143` | `SIGTERM`. |
