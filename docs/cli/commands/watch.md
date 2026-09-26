# `dx watch`

```text
dx watch [--clear] <build|test|run|lint|typecheck|format|check|fix> [scope...] [-- bazel-options...]
```

Reruns one command when files change. Every iteration reuses that command's
flags, scope handling, and exit codes. Scope is re-resolved each iteration.
Wrapped-command flags pass through per iteration.

- Wraps only `build`, `test`, `run`, `lint`, `typecheck`, `format`,
  `check`, and `fix`. Other commands fail with `not watchable`.
- `--clear`: clear the screen each iteration.
- Local only. Refuses `CI=true`. No daemon, cache, or remote execution.

```sh
bazel run //cli/cli:dx -- watch build //...
bazel run //cli/cli:dx -- watch --clear test --here
```
