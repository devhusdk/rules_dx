# `dx check`, `dx fix`, And `dx clean`

```sh
bazel run @rules_dx//:dx -- check //...
bazel run @rules_dx//:dx -- fix //...
bazel run @rules_dx//:dx -- clean
```

## `dx check` And `dx fix`

```text
dx check [--here] [--affected] [--check] [--explain] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...] [-- bazel-options...]
dx fix [--here] [--affected] [--check] [--apply] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...] [-- bazel-options...]
```

Before any phase, `dx check` and `dx fix` fail when your change adds build
output: a new file under `target/`, `bazel-*`, `node_modules/`, `__pycache__/`,
`.gradle/`, or a `build/` directory outside a source tree, or a file over
1 MiB. The error names each file and exits `1`.

- Your change is untracked unignored files, staged and unstaged edits, and
  commits since the merge base with the default branch (`origin/HEAD`,
  `origin/main`, `main`, `origin/master`, `master`).
- Ignored files never count.
- A file that already exists at the merge base is only checked for size.
- List a path or a `dir/` prefix you want to keep in `.dx-large-files`, one
  per line. Listed paths skip both rules.
- Symlinks and Git LFS files are skipped.
- With no default branch or merge base (a shallow clone), only the working
  tree is checked. In text output `dx` prints a note on stderr.
- Outside a Git checkout the check is skipped.
- If Git fails, the command exits `2` with code `build_output_git_failed`.

`dx check` also compares the dx-generated `third_party/rules_dx/` overrides
with this `dx` before any phase runs. A missing or changed generated file
fails with code `overrides_stale` and exit `1`; run `tools/dx init --apply` to
regenerate. A workspace that has not added the include gets a note on stderr
and continues. rules_dx itself, and workspaces without a `rules_dx`
`bazel_dep`, are skipped.

Runs `generate`, then `format`, then `lint`, then `typecheck` in order.
`--affected` limits `format`, `lint`, and `typecheck` to targets your git changes reach (see
[Scope Defaults](scope-defaults.md)), runs `generate` on the packages of those
targets, or on the whole repo when the scope widens to `//...`, and makes
`dx check` run the affected tests as a last `test` phase. It skips that phase when no test is affected. With
`--output=json`, `affected.tests` on `command_started` lists the tests dx
chose. With nothing affected, `dx check --affected` and `dx fix --affected`
run no phase, `generate` included, and skip the build-output check. They print
`no affected targets` and exit `0`. With `--output=json` they emit only
`command_started` (with `affected`) and `command_finished`; `mode` is `check`
unless `--apply` is given. Affected targets that do not fit the host platform
are skipped.
Every phase, including `test`, passes the same Bazel settings:
`--@rules_dx//config:clippy_output_diagnostics=true`
and `--@rules_dx//config:rustc_output_diagnostics=true`. Another value for one of them,
or for the matching `@rules_rust//rust/settings` flag, fails before Bazel starts with exit `2`.
dx spells them with the name your MODULE.bazel gives rules_dx, for example
`--@my_dx//config:clippy_output_diagnostics=true` when `repo_name = "my_dx"`.
`dx check --explain` saves one execution log for `dx explain`: each Bazel phase
(and the affected tests) writes its own log, and dx joins them into one
`.dx/explain/<n>.binpb.zst`, keeping the two newest. A `-- bazel-options` entry
that sets another execution log is refused.
`--check` passes through to every phase. `--fail-on` applies to `format`,
`lint`, and `typecheck`. `--report sarif=<dest>` merges the `lint` and
`typecheck` phases into one document. `generate` and `test` take neither. A
relative destination anchors at the workspace root; an absolute one writes as
given. Colliding destinations fail before anything runs, parents must exist,
and a failed write keeps the previous file bytes. `dx check` is always a check,
so `--check` is implied. `-` for stdout is rejected because the phases share
one document.

`dx check` and `dx fix` run every phase. A phase with findings or drift, a
failed build or tool, a target that does not converge, incomplete results, or
a report that cannot be written does not stop the next phase, so one run
reports every phase. The run still exits nonzero, and a later clean phase
never clears it. The run stops, and marks the remaining phases skipped, only
on a bad scope or configuration, when Bazel cannot run (it fails to launch, is
killed by a signal, or the disk is full), or on a cancel. A phase whose only
failures are compile errors does not stop `dx check`, but it stops `dx fix`:
the remaining phases are skipped (after `fix --apply` applied changes, `generate` still reruns once) with
`<phase> skipped after <phase> failed to compile`. The skipped phases are named
in the JSON `umbrella_phase_skipped` notice events. In text output, a run with a failed
phase ends with a `dx: phases:` line on stderr that says how each phase ended.

Every phase that ran and owed a capture must leave a readable SARIF document
behind. A missing, unreadable, or malformed capture is a `collection_failed`
error: the run fails and the report is marked incomplete. Findings from the
phases that did land are still merged. The written line names every phase
that failed, was skipped, or lost its capture.

A phase that fails still keeps the findings it wrote: its SARIF is merged and
the report is marked incomplete. A phase that only reports findings leaves the
report complete. The run exits with the code of the phase that stopped it, or
of the first failed phase, even when the report cannot be written. A
failed phase that wrote nothing adds no `collection_failed` error.

Output: `--output text|diff|json`. JSON keeps one `command_started` and one
`command_finished` for the run. Each phase sits between a `phase_started` and
a `phase_finished`, and its events carry `command`, `phase`, and a
`correlation` such as `check/format`. `dx fix --apply` verification phases use
`verify/<phase>`. When `format`, `lint`, or `typecheck` wrote a file,
`dx fix --apply` runs `generate` again after `typecheck` so new sources and
imports reach the BUILD files; that phase uses `rerun/generate`. A
`verify/<phase>` that writes events dx cannot read adds a
correlated `collection_failed` error and fails the run. A phase that reported findings adds a correlated `notice`
(`umbrella_phase_findings`), a skipped phase adds a correlated `notice`
(`umbrella_phase_skipped`), and a capture the run could not collect adds a
correlated `error`. `--include-edits` adds full edits to JSON `change` events
and needs `--output json`.

The run's `command_finished` carries `actions`, the Bazel action counts summed
over every phase, including `verify/<phase>` and the test phase. Each
`phase_finished` carries the counts of its own phase. Text output prints one
`dx: <total> actions, <executed> executed, <cached> cached` line on stderr for
the whole run.

Findings keep the tool's labels, notes, help, and related locations. Text
prints them under the finding, JSON adds an ordered `related` list, and SARIF
adds `relatedLocations`. They are never counted as findings or applied as
edits.

SARIF results carry `partialFingerprints` (`dx/v1`), `ruleIndex`, and `fixes` for machine-applicable replacements. Runs set `columnKind` to `unicodeCodePoints`.

`dx check` only reports. `dx fix` checks by default; `dx fix --apply`
writes validated fixes, then re-checks read-only every phase up to the last
one that changed or refused a file. The re-check runs only when every phase
passed. When no phase changed anything, no re-check runs. `dx fix --apply` succeeds only when that re-check passes;
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
first failed phase sets the code.

The `test` phase also keeps Bazel's code. `3` with every test result collected
means some affected tests failed: the run counts that as findings and its
results are complete. `3` or `1` with results incomplete is an incomplete
phase, not findings: the reason is `test failed` and the results are
incomplete. Results are incomplete when the build failed, a result has no
`test.xml`, a `test.xml` is missing, truncated or invalid, Bazel announces a
result it never reports, a target has a summary but no result, or the build
events end before Bazel's last event. `2` and `4` mean the scope selected no
runnable tests.

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
nothing. `--check` is that default: it reports and deletes nothing. Pass `--apply` to prune it. A generation a live reader still
holds is never pruned. When process observation is unavailable (`/proc` cannot be listed, or any
process cannot be read, such as one owned by another user or blocked by ptrace
restrictions), dx keeps unobserved state and reports it; pass `--prune-unobserved` to prune it
anyway; this still keeps state used by readable processes and never overrides a held lease. Never touches Bazel outputs; run `bazel clean` for those. Takes no scopes. It manages setup records and generations, and `--apply` also deletes the execution logs `--explain` saved under `.dx/explain` (a `dx clean: pruned <n> execution logs from .dx/explain` line, or a `pruned` notice with that path in JSON); it leaves other `.dx` entries alone; `dx worktree prune --apply --bases` reclaims tagged rebuildable caches in free slots.

Output: `--output text|json`. `--quiet` silences text output.

Text, check mode, when something can be pruned or is preserved:

- `dx clean: reclaimable managed state`
- `process observation unavailable: unobserved state is preserved, never assumed idle` when unobserved state is preserved
- `process observation unavailable: unobserved state pruned under --prune-unobserved` when unobserved state is pruned by `--prune-unobserved`
- `prune setup record: .dx/setups/<hex> (<n> bytes)`
- `prune generation: .dx/<environments|generated>/<hex> (<n> bytes)`
- `preserve current: .dx/setups/<hex>`, or `no current selection`
- `preserve unobserved setup record: .dx/setups/<hex>` and
  `preserve unobserved generation: .dx/<environments|generated>/<hex>`
- `refuse unmanaged path: <path>`
- `reclaimable total: <n> bytes`
- ``run `dx clean --apply` to prune it``, or
  ``run `dx clean --apply --prune-unobserved` to prune it anyway`` when only
  preserved state is listed

When nothing can be pruned or preserved it prints `dx clean: nothing to prune`.

Text, `--apply`: `dx clean: pruned <n> setup records and <m> generations
(<b> bytes reclaimed)`, followed by `; preserved <k> leased generations`
when a live reader holds one. When process observation is unavailable it
prints `dx clean: preserved <n> unobserved setup records and <m> unobserved
generations (process observation unavailable)` and
``run `dx clean --apply --prune-unobserved` to prune it anyway``. With
nothing to prune it prints `dx clean: nothing to prune`.

JSON: `command_started` has mode `check`, or `default` with `--apply`.
Then one `operation` event, `collect`. Then one info `notice` per path.
Each notice has a `path` under `.dx/` and the byte size in `message`:

- `clean_planned`: would be pruned. Check mode.
- `clean_preserved_unobserved`: kept because process observation is
  unavailable. Check mode and `--apply`, when process observation is
  unavailable.
- `clean_pruned`: pruned. `--apply`.
- `clean_preserved_leased`: kept because a live reader holds it.
  `--apply`.

A failure emits an `error` event with code `clean_failed`.
`command_finished` ends the stream.

Exit codes: `0` success, `2` usage errors including any scope,
`1` drift or a prune failure. State kept only because process observation
is unavailable exits `0`.

```sh
bazel run @rules_dx//:dx -- clean --check
bazel run @rules_dx//:dx -- clean --apply
bazel run @rules_dx//:dx -- clean --apply --prune-unobserved
```
