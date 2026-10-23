# Build, Test, And Coverage Commands

```sh
bazel run @rules_dx//:dx -- build //...
bazel run @rules_dx//:dx -- test //...
bazel run @rules_dx//:dx -- coverage //...
```

No scope means `//...`. Pass a label, pattern, file, or dir to narrow it.
Use `--here` for the current dir tree, or `--affected` for the targets your
git changes reach (see [Scope Defaults](scope-defaults.md)). File scopes
resolve to the owning targets. Args after `--` go to Bazel unchanged.

Ctrl-C, `SIGTERM` or `SIGHUP` stops a run. A `SIGHUP` that was ignored when
dx started, as under `nohup`, stays ignored. Bazel or the app gets the signal.
Outside a terminal, every process dx started gets it too, and any still running after
10 seconds is killed. A stopped run exits by that signal and emits no
`command_finished` event. The shared Bazel server keeps running.

Outside a terminal, if dx is killed with `SIGKILL`, Linux stops the process dx
started. Anything else in that process group keeps running until the next dx
run in that worktree stops it, prints `dx: stopped leftover process group <id>
from dx run <pid>, which is gone`, and with `--output=json` emits a `notice`
event with code `orphan_process_group_stopped`, after `command_started`. On
macOS the next run stops the whole tree. In a terminal, a `SIGKILL` to dx alone
leaves its children running. A dx that exits on its own never leaves anything
for the next run to stop.

## `dx build` And `dx test`

```text
dx build [--here] [--affected] [--debug|--release] [--report failures=<path>] [scope...] [-- bazel-options...]
dx test [--here] [--affected] [--debug|--release] [--report junit=<path>] [--report failures=<path>] [scope...] [-- bazel-options...]
```

Builds or tests the scope. `--debug` uses the `dx_debug` profile,
`--release` uses `dx_release`. No flag uses `dx_dev`. The two flags conflict.
Extra `--config` values pass through after the dx profile in their original
order. Only a conflicting dx profile is rejected.

Output: `--output text|json`. Reports: `dx test` writes
`--report junit=<path>` JUnit reports. Repeat the flag for more files. Use `-`
for stdout. A relative path anchors at the workspace root; an absolute one writes
as given. Colliding destinations fail before anything runs, parents must exist,
and a failed write keeps the previous file bytes. `dx test --output=json` adds one
`test_outcome` event per test result, with its Bazel status, run, shard,
attempt, cached state, duration, case counts, artifact counts, and whether its
evidence is complete.

`dx build`, `dx test` and `dx coverage` report each failure Bazel records
when it exits nonzero. `--output=text` prints one `dx: build failure:` line
per failure on stderr, with its rustc diagnostics under it.
`--output=json` adds one `build_failure` event per failure, before
`command_finished`. Each event has:

- `record_version`: `1`. New fields are only ever added.
- `target`, `configuration`, `target_kind` (`test`, `other` or `unknown`).
- `stage`: `analysis`, `tool_acquisition`, `compilation`, `linking`,
  `test_execution`, `other` or `unknown`.
- `category`: `analysis_failed`, `loading_failed`, `tool_unavailable`,
  `tool_missing`, `compile_failed`, `link_failed`, `test_execution_failed`,
  `action_failed`, `target_not_built`, `skipped`, `cancelled` or `aborted`.
- `action`: `mnemonic`, `primary_output` and, when known, `exit_code`.
- `tool`: `rustc` for Rust actions.
- `message`: the first line of Bazel's reason, for failures with no action.
- `diagnostics`: `status`, `total` and up to 50 `items` with `severity`,
  `code`, `message`, `path`, `line` and `column`.

Anything Bazel did not report reads `unknown`. An analysis failure has no
action, so its action fields are `unknown`. A test target that fails to build
emits a `build_failure` with `target_kind` `test` and no test case in the JUnit
report. It also gets a `test_outcome` with outcome `failed_to_build`, no case
counts, and `status` `FAILED_TO_BUILD` only when Bazel reports that summary.
The run's results are incomplete. Environment variables and command lines are
never reported.

Diagnostics come from rustc's JSON diagnostics in the failed action's stderr.
`status` is `parsed` when diagnostics were read, `none` when the stderr held
none, `unavailable` when the stderr is remote or unreadable, and `unknown` for
non-Rust actions. A Rust action whose diagnostics include a `linking with`
error is reported as `linking`. Pass a rustc JSON error format through Bazel
to get diagnostics, for example
`-- --@rules_rust//rust/settings:error_format=json`.

The failures report: `--report failures=<path>` on `dx build`, `dx test` and `dx coverage` writes
one JSON file with the failures of this run. It follows the same path rules
as the other reports and may share a run with `junit` or `lcov`. Use `-` for
stdout in text mode. The file is written on success too, with an empty
`failures` list. A failed write is a `report_failed` error. It fails a run Bazel
passed and never changes Bazel's own code.

The file has:

- `record_version`: `1`, `exit_code`: Bazel's code.
- `complete`: `false` when the build events end early or cannot be read, with
  the reason in `problem`.
- `truncated`: `true` when any stderr was cut at a limit.
- `failures_omitted`: failures past the first 200.
- `failures`: the `build_failure` fields above, each with a `stderr` object.

`stderr` has `status`, `bytes`, `truncated`, `redacted_lines` and, for
captured output, `text`. `status` is:

- `structured` for Rust actions whose diagnostics were parsed. Read
  `diagnostics` instead.
- `captured` or `truncated` for other actions. Each failure keeps at most
  64 KiB and the file at most 1 MiB. Later failures read `omitted_limit`.
- `remote` when Bazel kept the stderr on a remote cache. It is never fetched.
- `absent` when Bazel named no stderr, `unreadable` when the local file is
  gone.

Only this run's failed actions are read. No directory is scanned and no
history is kept. Lines that start a command line (`(cd ` or `exec env`) are
dropped and counted in `redacted_lines`. Environment variables and command
lines are never reported. Positions come only from rustc diagnostics.

```sh
bazel run @rules_dx//:dx -- build --report failures=build-failures.json //...
bazel run @rules_dx//:dx -- test --report junit=junit.xml --report failures=failures.json
```

If Bazel's event file ends early or cannot be read, dx still returns Bazel's
code and reports a `build_events_incomplete` notice.

Exit codes: `0` success. `2` usage or scope errors. `1` operational
failures. Bazel failures keep Bazel's code. Results are incomplete when a
result has no `test.xml`, a `test.xml` is missing, truncated or invalid,
Bazel announces a result it never reports, a target has a summary but no
result, or the build events end before Bazel's last event. Another test's
evidence never makes up for it. Incomplete results fail the run, even when
Bazel passes. `--output=text` prints `dx: incomplete_results: <reason>` on
stderr, `--output=json` emits an `incomplete_results` error, the JUnit report
gets a `dx.infrastructure` error case, and `results_complete` reads `false`.
Failing tests with every result collected keep Bazel's code `3` and read
`results_complete: true`. Other Bazel failures read `false`.

```sh
bazel run @rules_dx//:dx -- build //cli/...
bazel run @rules_dx//:dx -- test --here
bazel run @rules_dx//:dx -- test --affected
bazel run @rules_dx//:dx -- test //... -- --jobs=4
bazel run @rules_dx//:dx -- test //cli/process/... -- --test_arg=workflow_argv --test_filter=workflow
```

Args after `--` reach the test binary through Bazel. `dx test` and
`dx coverage` accept repeatable `--test_arg` tokens, in equals and
two-token form, with spaces and Unicode kept verbatim. `--test_filter`
stays a Bazel option with framework-dependent semantics. Other commands
reject `--test_arg`; run those cases with `bazel` directly.

Tests run once by default. Only tests marked flaky in their build
definition retry, up to three attempts. Diagnose one test with retries
through `dx test <label> -- --flaky_test_attempts=3`, or through
`bazel test --config=retry <label>`. A test that passes after retry is
reported as `passed_after_retry` with its attempt number.

## `dx run`

```text
dx run [--debug|--release] <label...> [-- args...]
```

Builds the runnable targets in scope, then launches them in scope
order. Explicit labels and patterns run sequentially. File and directory
scopes must resolve to exactly one runnable target. Args after `--` go to
the app. The default profile is development; `--release` selects release.
`dx run` works with `CI=true`. `dx run --apply` is a usage error.

Output: `--output text|json`. `dx run` has no report format.

Exit codes: `0` success, `2` usage or scope errors, `1` no runnable target or
a launch failure. The app's own code is the exit code.

```sh
bazel run @rules_dx//:dx -- run @rules_dx//:dx -- --help
bazel run @rules_dx//:dx -- run --release @rules_dx//:dx -- --help
```

## `dx deploy`

```text
dx deploy [--apply] [--debug|--release] <label> [-- args...]
```

Builds one deployable target without publishing it. Pass `--apply` to run
the deployment. Takes exactly one label. No flag uses `dx_release` here.
Args after `--` go to the app, and only reach it on an `--apply` launch.

Output: text only.

Exit codes: `0` success, `2` usage or scope errors, `1` a build or launch
failure. The target's own code is the exit code. A passing check names
the `--apply` command that publishes.

`--affected` builds or tests the changed targets and their reverse
dependencies. `dx test --affected` and `dx coverage --affected` run only the
affected test targets. With nothing affected they print `no affected targets`
and exit `0`. Usage errors exit `2`.

## `dx coverage`

```text
dx coverage [--here] [--affected] [--min-coverage <percent>] [--report lcov=<path>] [--report failures=<path>] [scope...] [-- bazel-options...]
```

Collects LCOV coverage over the scope. `--min-coverage` fails below that
percent. Without it, coverage collects without enforcing.

Output: `--output text|json`. Reports: `--report lcov=<path>` writes
combined LCOV. Repeat the flag for more files. Use `-` for stdout. A relative
path anchors at the workspace root; an absolute one writes as given. Colliding
destinations fail before anything runs, parents must exist, and a failed write
keeps the previous file bytes.
`dx coverage --output=json` adds one `test_outcome` event per test result,
with its Bazel status, run, shard, attempt, cached state, duration, artifact
counts, and whether its evidence is complete.

`--report failures=<path>` works here too, as described above.

Exit codes: `0` success. `2` usage or scope errors. `1` operational
failures, coverage below minimum, or incomplete coverage. Bazel failures
keep Bazel's code. Coverage is incomplete when Bazel reports a
`coverage.dat` artifact that is missing or invalid, when no coverage
artifacts are reported, or when the build events are incomplete as for
`dx test`. Incomplete coverage fails the run, blocks
`--min-coverage`, and marks a written LCOV report incomplete. A retried
test counts only its final attempt. Tests without coverage outputs are
not expected to produce coverage. Empty but valid `coverage.dat` files
are accepted and contribute no lines.

```sh
bazel run @rules_dx//:dx -- coverage //...
bazel run @rules_dx//:dx -- coverage --min-coverage 100 //...
```

Coverage ignores use `LCOV_EXCL_LINE` or `LCOV_EXCL_START` / `LCOV_EXCL_STOP`
with a short `reason:` and `issue:`. Bare ignores without a reason fail the
gate.

## Line And Branch Metrics

The coverage gate enforces exact executable-line coverage. Branch outcomes
are measured separately and never change the gate. Each file with branch
records reports taken and recorded counts with its language. Files without
branch records report unavailable instead of a rate. A declared branch
summary that does not match its records fails the report as incomplete.
