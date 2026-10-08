# `dx check`, `dx fix`, And `dx clean`

```sh
bazel run @rules_dx//:dx -- check //...
bazel run @rules_dx//:dx -- fix --apply //...
bazel run @rules_dx//:dx -- clean
```

## `dx check` And `dx fix`

```text
dx check [--here] [--check] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...] [-- bazel-options...]
dx fix [--here] [--check] [--apply] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...] [-- bazel-options...]
```

Runs `format`, then `lint`, then `typecheck`, then `generate` in order. Stops
on the first failure. `--fail-on` and `--report sarif=<dest>` pass through to
each phase. `dx check` is always a check, so `--check` is implied; `dx fix`
reads the same way until `--apply` authorizes writes. Reports
merge the `lint` and `typecheck` phases; `-` for stdout is rejected because
the phases share one document.

Every phase that ran and owed a capture must leave a readable SARIF document
behind. A missing, unreadable, or malformed capture is a `collection_failed`
error: the run fails and the report is marked incomplete. Findings from the
phases that did land are still merged. The written line names every phase
that failed, was skipped, or lost its capture.

Output: `--output text|diff|json`. JSON adds one correlated `operation` event
per phase (`check/format`), a correlated `notice` per skipped phase, and a
correlated `error` per capture the run could not collect.

`dx check` only reports. `dx fix` reports too and writes only with `--apply`.
With `--apply`, `dx fix` writes the fixes and then runs every phase again with
read-only checks. The run passes only when those checks pass. A check that
still reports a finding, a change that was not applied, a skipped check, and a
failed phase all keep the run failing. JSON names each verification phase with
a correlated `operation` event (`fix/verify/format`) and each check that never
ran with a correlated `notice`.

Exit codes: `0` success, `2` usage or scope errors, `1` a phase failed, a
capture could not be collected, a report could not be written, or a
verification check failed. The failing phase's code wins, so a `generate`
phase keeps Bazel's code.

```sh
bazel run @rules_dx//:dx -- check //...
bazel run @rules_dx//:dx -- fix --apply --here
```

## `dx clean`

```text
dx clean [--apply] [--dry-run] [--bazel] [--prune-unobserved]
```

Prunes unselected managed state under `.dx`. Never touches Bazel outputs
unless `--bazel` also runs `bazel clean`. Takes no scopes.
`--dry-run` only lists what would go.

Output: `--output text|json`.

Exit codes: `0` success, `2` usage errors including any scope, `1` a prune
or launch failure. With `--bazel`, a `bazel clean` failure keeps Bazel's
code.

```sh
bazel run @rules_dx//:dx -- clean --dry-run
```
