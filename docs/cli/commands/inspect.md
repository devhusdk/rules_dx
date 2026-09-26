# Inspect Wrappers

Thin wrappers over `bazel query`. They canonicalize scopes and reuse target
resolution. No custom graph engine.

```text
dx owners [--configured] <scope>...
dx deps [--configured] <scope>...
dx why [--configured] <file> <label>
```

- `dx owners`: list owning targets for files. Needs at least one scope.
- `dx deps`: list dependencies of targets. Needs at least one scope.
- `dx why`: show one path from a file owner to a label. Takes exactly one
  file plus one label.
- `--configured`: use `bazel cquery` instead of `bazel query`.
- Output is text or JSON only.

```sh
bazel run //cli/cli:dx -- owners cli/cli/src/main.rs
bazel run //cli/cli:dx -- deps //cli/cli:dx
bazel run //cli/cli:dx -- why cli/cli/src/main.rs //cli/cli:dx
```
