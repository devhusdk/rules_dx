# `dx watch`

```text
dx watch [--apply] <build|test|run|lint|typecheck|format|check|fix> [scope...]
```

Reruns one command when files change. Scope is re-resolved each iteration.
The wrapped command runs with the watch invocation mode: pass `--apply`
when the wrapped effect needs it, such as `dx watch --apply run //app:bin`.

- Wraps only `build`, `test`, `run`, `lint`, `typecheck`, `format`,
  `check`, and `fix`. Other commands fail with `not watchable`.
- Takes `--apply` only. `--check`, `--here`, `--debug`, `--release`,
  and args after `--` do not apply.
- Local only. Refuses `CI=true`. No daemon, cache, or remote execution.
- Ignores changes under `.dx/` and `bazel-*`, and to `dx.local.toml`.

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
bazel run @rules_dx//:dx -- watch --apply run //app:bin
```
