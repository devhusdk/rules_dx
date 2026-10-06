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

Output: `--output text|json`.

Exit codes: `0` success, `2` usage or scope errors, `1` a query failed or
`dx why` found no owner.

```sh
bazel run @rules_dx//:dx -- owners cli/cli/src/main.rs
bazel run @rules_dx//:dx -- deps @rules_dx//:dx
bazel run @rules_dx//:dx -- why cli/cli/src/main.rs @rules_dx//:dx
```
