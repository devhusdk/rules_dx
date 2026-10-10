# `dx verify`

```text
dx verify <set> [-- bazel-options ...]
```

Runs one committed verification set from the workspace `dx.verify.toml` in
order. The file carries `schema = 1` and one `[sets.<name>]` table per set;
each step names a `command` (`format`, `lint`, `typecheck`, `generate`,
`build`, `test`, or `coverage`), optional `scopes`, optional
`bazel_options`, and optional `required` (default `true`). Steps run through
the same phases as their commands. A step without `scopes` runs repo-wide.
Step options run first, then the invocation `--` options trail every step.
`dx check` stays quality-only; `dx verify` is the set runner that also runs
`build`, `test`, and `coverage` steps.

A required failure stops the set: later steps never run and stay out of any
success claim. An optional failure is reported and the set continues. A
green set means every required step passed; skipped required steps can never
be green. Set validation is noninteractive and never writes sources, runs
deployments, or restores history.

Output: `--output text|json`. JSON starts with `command_started`, adds one
correlated `operation` event per step (`verify/build`), a correlated `notice`
per skipped step, a correlated `error` per optional failure, and ends with
`command_finished`.

Exit codes: `0` success, `2` usage errors including a missing set file,
an unknown set, or a malformed step, `1` a required step failed or a
required step never ran. A failing required step keeps Bazel's code.

```sh
bazel run @rules_dx//:dx -- verify pre-pr
bazel run @rules_dx//:dx -- verify pre-pr --output=json
bazel run @rules_dx//:dx -- verify pre-pr -- --jobs=4
```
