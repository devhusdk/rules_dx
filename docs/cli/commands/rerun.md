# `dx rerun`

```text
dx rerun <receipt> [-- bazel-options ...]
```

Reruns the failed test targets recorded in a `receipt.json` written by an
earlier `dx test --run-output <dir>` or `dx coverage --run-output <dir>`
run. The receipt sits beside `manifest.json` in the run-output child
directory and records the request and selection, the workspace and input
identity, the safe configured options, the observed tools, the per-target
outcomes, and the manifest reference. Secret-bearing option values are
withheld from the receipt, never persisted.

The rerun executes the recorded failed targets through the existing test
path on the current workspace only. Recorded safe Bazel options run first,
then the invocation `--` options trail under native precedence. A withheld
option is reported and never replayed; resupply it after `--` when the
rerun needs it. The recorded profile (`dx_debug`, `dx_dev`, `dx_release`)
is preserved; a coverage receipt reruns through the test path without
re-applying any coverage threshold. Source restoration, branch fetch, and
deployment are not part of this command.

A rerun is new evidence, never a historical reproduction. A changed
workspace, changed inputs, changed dx pin, or withheld option is reported
explicitly and the run continues. A renamed target fails through the
ordinary test path. A receipt that records an incomplete run refuses with
`rerun_receipt_incomplete`. When the receipt records no failed targets,
`dx rerun` reports nothing to rerun with `validation_performed=false`
(`results_complete: false` in JSON) and runs no tests.

Output: `--output text|json`. JSON starts with `command_started`, adds one
correlated `notice` per changed workspace, changed inputs, withheld option,
and changed tool pin, forwards the delegated test stream verbatim, and ends
with `command_finished`.

Exit codes: `0` success including nothing to rerun, `2` usage errors
including a missing, unreadable, malformed, or unsupported receipt,
`1` a receipt that records an incomplete run or a rerun whose tests failed.
A failing rerun keeps the test run's exit code.

```sh
bazel run @rules_dx//:dx -- rerun out/dx-run-1-2/receipt.json
bazel run @rules_dx//:dx -- rerun out/dx-run-1-2/receipt.json --output=json
bazel run @rules_dx//:dx -- rerun out/dx-run-1-2/receipt.json -- --jobs=4
```
