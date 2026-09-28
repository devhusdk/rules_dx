# `dx watch`

```text
dx watch <build|test|run|lint|typecheck|format|check|fix> [scope...]
```

Reruns one command when files change. Scope is re-resolved each iteration.

- Wraps only `build`, `test`, `run`, `lint`, `typecheck`, `format`,
  `check`, and `fix`. Other commands fail with `not watchable`.
- Takes no per-command flags. `--check`, `--here`, `--debug`, `--release`,
  and args after `--` do not apply.
- Local only. Refuses `CI=true`. No daemon, cache, or remote execution.

Exit codes: `0` success, `2` usage error, `1` operational failures.

```sh
bazel run //cli/cli:dx -- watch build //...
bazel run //cli/cli:dx -- watch test //cli/...
```
