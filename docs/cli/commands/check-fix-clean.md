# `dx check`, `dx fix`, And `dx clean`

```sh
bazel run @rules_dx//:dx -- check //...
bazel run @rules_dx//:dx -- fix //...
bazel run @rules_dx//:dx -- clean
```

## `dx check` And `dx fix`

```text
dx check [--here] [--check] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...] [-- bazel-options...]
dx fix [--here] [--check] [--apply] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...] [-- bazel-options...]
```

Runs `format`, then `lint`, then `typecheck`, then `generate` in order. Stops
on the first failure. `--fail-on` and `--report sarif=<dest>` pass through to
each phase. `dx check` is always a check, so `--check` is implied. Reports
merge the `lint` and `typecheck` phases; `-` for stdout is rejected because
the phases share one document. Relative destinations resolve under the
workspace root; absolute destinations are used as written. Every report needs
its own file; missing parent directories fail the run.

Every phase that ran and owed a capture must leave a readable SARIF document
behind. A missing, unreadable, or malformed capture is a `collection_failed`
error: the run fails and the report is marked incomplete. Findings from the
phases that did land are still merged. The written line names every phase
that failed, was skipped, or lost its capture.

Output: `--output text|diff|json`. JSON adds one correlated `operation` event
per phase (`check/format`), a correlated `notice` per skipped phase, and a
correlated `error` per capture the run could not collect.

`dx check` only reports. `dx fix` checks by default; `dx fix --apply`
writes validated fixes, then verifies the composed tree with a read-only
check pass. `dx fix --apply` succeeds only when that verification passes;
remaining findings or a failed verification stay visible and fail the run.

Exit codes: `0` success, `2` usage or scope errors, `1` a phase failed, a
capture could not be collected, the post-apply verification failed, or a
report could not be written. The failing phase's code wins, so a `generate`
phase keeps Bazel's code.

```sh
bazel run @rules_dx//:dx -- check //...
bazel run @rules_dx//:dx -- fix --here
bazel run @rules_dx//:dx -- fix --apply //...
```

## `dx clean`

```text
dx clean [--check] [--apply] [--dry-run] [--bazel] [--prune-unobserved]
```

Reports unselected managed state under `.dx` by default and deletes
nothing. Pass `--apply` to prune it. Never touches Bazel outputs unless
`--bazel` is combined with `--apply` to also run `bazel clean`. Takes no
scopes. `--dry-run` only lists what would go and always succeeds.

Output: `--output text|json`.

Exit codes: `0` success, `2` usage errors including any scope, `1` drift, a
prune or launch failure. With `--bazel`, a `bazel clean` failure keeps
Bazel's code.

```sh
bazel run @rules_dx//:dx -- clean --dry-run
bazel run @rules_dx//:dx -- clean --apply
```
