# `dx watch`

```text
dx watch [--apply] [--debug|--release] [flags] <build|test|run|lint|typecheck|format|check|fix> [scope ...] [-- bazel-options|--app-args ...]
```

Reruns one command when files change. Scope is re-resolved each iteration.
Flags go to the wrapped command and are checked as `dx <command>` checks
them: `dx watch test --release`, `dx watch lint --report sarif=out.sarif`.
A flag the wrapped command rejects fails before watching starts. Short
options work wherever the long option does. Bare lint, typecheck, format, and
fix only check; pass `--apply` to write on every iteration. Args after `--`
reach the wrapped command unchanged.

- Wraps only `build`, `test`, `run`, `lint`, `typecheck`, `format`,
  `check`, and `fix`. Other commands fail with `not watchable`.
- Takes `--output`, `--check`, `--apply`, `--fail-on`, `--report`,
  `--include-edits`, `--here`, `--affected`, `--debug`, and `--release`, plus
  the global flags, when the wrapped command takes them.
- `--here` resolves once from the current directory. `--affected` re-resolves
  each iteration.
- Local only. Refuses `CI=true`. No daemon, cache, or remote execution.
- Ignores changes under `.dx/`, `.git/`, and `bazel-*` inside the workspace. Parent directories with those names do not matter. Also ignores the wrapped command's `--report` files and their temp files, matched by path relative to the workspace. Edits to `dx.toml`, `dx.local.toml`, `BUILD` and `MODULE.bazel` files rerun the command.

Except under `run`, changes made while the command runs queue one more
iteration. Iterations never overlap. A failing command does not stop the watch.
Each iteration takes a build slot and, on low disk, drops free slots' output
bases first. Watch holds none while it waits for changes. See
[Build Slots](README.md#build-slots) and [Disk Space](README.md#disk-space).

`watch run` restarts the app on each change. The running app and every
process it started get `SIGTERM`. Any still running after 10 seconds is
killed. The replacement builds and launches only after the old app is gone.
An app that exits on its own is relaunched on the next change. The app gets
no stdin.

Output: `--output text|diff|json`, when the wrapped command takes that mode.
Each iteration writes the wrapped command's own output. Watch adds one line
per iteration and one line per trigger, naming the first changed absolute path.
`--quiet` drops these lines. With `--output json`, `--output diff`, or
`--report <format>=-`, they go to stderr so stdout holds only the wrapped
command's output: under JSON, one event stream per iteration, each ending in
`command_finished`. For a restarted app, `command_finished` reports `143`.

`scope=` shows the given scopes, `//...` when none are given, or `--affected`.

```text
watch:test:debounce=200ms scope=//... iteration=1
watching (Ctrl-C to stop)
changed /work/repo/src/main.rs (+1 more)
watch:test:debounce=200ms scope=//... iteration=2
stopped
```

Ctrl-C, `SIGTERM` or `SIGHUP` stops the watch. No new iteration starts after the
signal. A running command gets the signal and is awaited first. Removing the
workspace directory stops the watch with `watch_failed`.

Watch runs until stopped and has no success exit.
Exit codes: `2` usage error, `not watchable`, or `CI=true`; `1` operational
failures such as `watch_failed`; `141` when stdout closes; `129` after
`SIGHUP`, `130` after Ctrl-C, `143` after `SIGTERM`.

```sh
bazel run @rules_dx//:dx -- watch build //...
bazel run @rules_dx//:dx -- watch test //cli/...
bazel run @rules_dx//:dx -- watch test --affected
bazel run @rules_dx//:dx -- watch lint --output json
bazel run @rules_dx//:dx -- watch run //app:bin
```
