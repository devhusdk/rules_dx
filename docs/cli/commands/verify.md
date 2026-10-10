# `dx verify`

```sh
bazel run @rules_dx//:dx -- verify pre-pr
bazel run @rules_dx//:dx -- verify pre-pr --output=json
bazel run @rules_dx//:dx -- verify pre-pr -- --jobs=4
```

## Sets

```text
dx verify <set> [-- bazel-options ...]
```

Runs one set from the committed `dx.verify.toml` file at the workspace root.
A set is an ordered list of steps. Each step names a command with explicit
scopes. Steps run in order. The first required failure stops the run, and the
required steps that never ran are reported as skipped. An optional failure
warns and the run continues.

A step command is one of the read-only validations: format, lint, typecheck,
generate, build, test, or coverage. Test and coverage steps fail when their
collected evidence is incomplete, even when Bazel itself passed. Options after
`--` append after each step's own Bazel options. The run never writes sources
and never launches deployments. Follow the hint a failing step prints, then
run the named `dx` command directly.

Output: `--output text|json`. JSON starts with `command_started`, emits one
correlated `operation` event per step, then ends with `command_finished`.

Exit codes: `0` every required step passed; `2` usage errors including an
unknown set or a malformed sets file; `1` a required step failed. The failing
step's code wins.

```sh
bazel run @rules_dx//:dx -- verify pre-pr
bazel run @rules_dx//:dx -- verify pre-pr --dry-run
```

## The Sets File

```toml
schema = 1

[sets.pre-pr]
steps = [
  { command = "format", scopes = ["//..."] },
  { command = "lint", scopes = ["//..."] },
  { command = "test", scopes = ["//..."] },
  { command = "build", scopes = ["//..."], required = false },
]
```

`schema` must be `1`. Each set holds a `steps` array. A step holds a
`command`, a `scopes` array, an optional `bazel_options` array, and an
optional `required` flag that defaults to `true`. Unknown keys and unknown
commands are usage errors. Scopes must be explicit and non-empty. Only the
committed file counts: local-only config never adds or weakens sets.
