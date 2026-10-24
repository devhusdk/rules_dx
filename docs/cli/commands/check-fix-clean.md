# `dx check`, `dx fix`, And `dx clean`

```sh
bazel run @rules_dx//:dx -- check //...
bazel run @rules_dx//:dx -- fix //...
bazel run @rules_dx//:dx -- clean
```

## `dx check` And `dx fix`

```text
dx check [--here] [--affected] [--check] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...] [-- bazel-options...]
dx fix [--here] [--affected] [--check] [--apply] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...] [-- bazel-options...]
```

Before any phase, `dx check` and `dx fix` fail when the checkout holds build
output: a file under `target/`, `bazel-*`, `node_modules/`, `__pycache__/`,
`.gradle/`, or a `build/` directory outside a source tree, or a file over
1 MiB. The error names each file and exits `1`. Only tracked and untracked,
unignored files count. List an oversized file you want to keep in
`.dx-large-files`, one path per line. Outside a Git checkout the scan is skipped.

Runs `format`, then `lint`, then `typecheck`, then `generate` in order.
`--affected` limits the first three to targets your git changes reach (see
[Scope Defaults](scope-defaults.md)), keeps `generate` on the whole repo, and
makes `dx check` run the affected tests as a last `test` phase. It skips that
phase when no test is affected. With `--output=json`, `affected.tests` on
`command_started` lists the tests dx chose. With nothing affected, `dx check --affected`
prints `no affected targets` and exits `0`.
Every phase, including `test`, passes the same Bazel settings:
`--@rules_dx//config:validate=false`, `--@rules_dx//config:clippy_output_diagnostics=true`
and `--@rules_dx//config:rustc_output_diagnostics=true`. Another value for one of them,
or for the matching `@rules_rust//rust/settings` flag, fails before Bazel starts with exit `2`.
`--fail-on` and `--report sarif=<dest>` pass through to each phase. A relative
destination anchors at the workspace root; an absolute one writes as given.
Colliding destinations fail before anything runs, parents must exist, and a
failed write keeps the previous file bytes. `dx check` is always a check, so
`--check` is implied. Reports merge the `lint` and `typecheck` phases; `-` for
stdout is rejected because the phases share one document.

`dx check` runs every phase against the unchanged workspace. A phase that
finds drift or ordinary findings does not stop the next one, so one run lists
every finding. The run still exits nonzero, and a later clean phase never
clears it. `dx check` stops, and marks the remaining phases skipped, when a
phase fails for any other reason: a bad scope or configuration, a tool or
build that cannot run, a target that does not converge, incomplete results, a
report that cannot be written, or a cancel. The skipped phases are named in
the JSON `notice` events and in the `dx: phases:` line in text output.
`dx fix` stops on the first failing phase.

Every phase that ran and owed a capture must leave a readable SARIF document
behind. A missing, unreadable, or malformed capture is a `collection_failed`
error: the run fails and the report is marked incomplete. Findings from the
phases that did land are still merged. The written line names every phase
that failed, was skipped, or lost its capture.

A phase that fails still keeps the findings it wrote: its SARIF is merged and
the report is marked incomplete. A phase that only reports findings leaves the
report complete. The run exits with the code of the phase that stopped it, or
of the first phase with findings, even when the report cannot be written. A
failed phase that wrote nothing adds no `collection_failed` error.

Output: `--output text|diff|json`. JSON keeps one `command_started` and one
`command_finished` for the run. Each phase sits between a `phase_started` and
a `phase_finished`, and its events carry `command`, `phase`, and a
`correlation` such as `check/format`. `dx fix --apply` verification phases use
`verify/<phase>`. A phase that reported findings adds a correlated `notice`
(`umbrella_phase_findings`), a skipped phase adds a correlated `notice`
(`umbrella_phase_skipped`), and a capture the run could not collect adds a
correlated `error`. `--include-edits` adds full edits to JSON `change` events
and needs `--output json`.

Findings keep the tool's labels, notes, help, and related locations. Text
prints them under the finding, JSON adds an ordered `related` list, and SARIF
adds `relatedLocations`. They are never counted as findings or applied as
edits.

`dx check` only reports. `dx fix` checks by default; `dx fix --apply`
writes validated fixes, then re-checks read-only every phase up to the last
one that changed or refused a file. When no phase changed anything, no
re-check runs. `dx fix --apply` succeeds only when that re-check passes;
remaining findings, a target that did not converge, or a failed verification
stay visible and fail the run.

When a phase finds changes, text prints
`N file(s) need changes. Apply them with: dx fix --apply ...` with the scope
you passed, `--affected` included. `next:` lines name `dx check` or `dx fix`
with that scope.

Exit codes: `0` success, `2` usage or scope errors, `1` a phase failed or
reported findings, a capture could not be collected, the post-apply
verification failed, or a report could not be written. A phase that stopped
the run keeps its code, so a `generate` phase keeps Bazel's code. Otherwise the
first phase with findings sets the code.

The `test` phase also keeps Bazel's code. `3` means the affected tests ran and
some failed: the run counts that as findings and its results stay complete. `1`
from the `test` phase means the build failed or test results were missing, so
the results are incomplete. `2` and `4` mean the scope selected no runnable
tests.

```sh
bazel run @rules_dx//:dx -- check //...
bazel run @rules_dx//:dx -- fix --here
bazel run @rules_dx//:dx -- check --affected
bazel run @rules_dx//:dx -- fix --apply //...
```

## `dx clean`

```text
dx clean [--check] [--apply] [--prune-unobserved]
```

Reports unselected managed state under `.dx` by default and deletes
nothing. Pass `--apply` to prune it. A generation a live reader still
holds is never pruned. Never touches Bazel outputs; run `bazel clean` for those. `--bazel` is
rejected. Takes no scopes.

Output: `--output text|json`.

Exit codes: `0` success, `2` usage errors including any scope or `--bazel`,
`1` drift or a prune failure.

```sh
bazel run @rules_dx//:dx -- clean --check
bazel run @rules_dx//:dx -- clean --apply
```
