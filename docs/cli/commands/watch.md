# `dx watch`

```text
dx watch [--apply] [--debug|--release] [flags] <build|test|run|lint|typecheck|format|check|fix> [scope ...] [-- bazel-options|--app-args ...]
```

Reruns one command when files change. Scope is re-resolved each iteration.
Flags go to the wrapped command and are checked as `dx <command>` checks
them: `dx watch test --release`, `dx watch lint --report sarif=out.sarif`.
A flag the wrapped command rejects fails before watching starts. Short
options work wherever the long option does. Bare format and fix stay in
check mode; pass `--apply` to write on every iteration. Args after `--` reach
the wrapped command unchanged.

- Wraps only `build`, `test`, `run`, `lint`, `typecheck`, `format`,
  `check`, and `fix`. Other commands fail with `not watchable`.
- Takes `--check`, `--apply`, `--fail-on`, `--report`, `--debug`, and
  `--release`, plus the global flags, when the wrapped command takes them.
- Local only. Refuses `CI=true`. No daemon, cache, or remote execution.
- Ignores changes under `.dx/` and `bazel-*`. Edits to `dx.toml`, `dx.local.toml`, `BUILD` and `MODULE.bazel` files rerun the command.

Each change runs the wrapped command again. A failing command does not stop
the watch. Ctrl-C stops it.

Output: `--output text`. One line per iteration, then one absolute path per
trigger.

```text
watch:test:debounce=200ms scope=//... iteration=1
watching (Ctrl-C to stop)
changed /work/repo/src/main.rs (+1 more)
watch:test:debounce=200ms scope=//... iteration=2
```

Exit codes: `0` success, `2` usage error, `1` operational failures.

```sh
bazel run @rules_dx//:dx -- watch build //...
bazel run @rules_dx//:dx -- watch test //cli/...
bazel run @rules_dx//:dx -- watch run //app:bin
```
