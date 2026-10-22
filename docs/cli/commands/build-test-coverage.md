# Build, Test, And Coverage Commands

```sh
bazel run @rules_dx//:dx -- build //...
bazel run @rules_dx//:dx -- test //...
bazel run @rules_dx//:dx -- coverage //...
```

No scope means `//...`. Pass a label, pattern, file, or dir to narrow it.
Use `--here` for the current dir tree. File scopes resolve to the owning
targets. Args after `--` go to Bazel unchanged.

Ctrl-C or `SIGTERM` stops a run. Bazel or the app gets the signal. Outside
a terminal, every process dx started gets it too, and any still running after
10 seconds is killed. A stopped run exits by that signal and emits no
`command_finished` event. The shared Bazel server keeps running.

## `dx build` And `dx test`

```text
dx build [--here] [--debug|--release] [scope...] [-- bazel-options...]
dx test [--here] [--debug|--release] [--report junit=<path>] [scope...] [-- bazel-options...]
```

Builds or tests the scope. `--debug` uses the `dx_debug` profile,
`--release` uses `dx_release`. No flag uses `dx_dev`. The two flags conflict.
Extra `--config` values pass through after the dx profile in their original
order. Only a conflicting dx profile is rejected.

Output: `--output text|json`. Reports: `dx test` writes
`--report junit=<path>` JUnit reports. Repeat the flag for more files. Use `-`
for stdout. A relative path anchors at the workspace root; an absolute one writes
as given. Colliding destinations fail before anything runs, parents must exist,
and a failed write keeps the previous file bytes. `dx build` has no report
format. `dx test --output=json` adds one
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
emits a `build_failure` with `target_kind` `test`, no `test_outcome` event and
no test case in the JUnit report. Environment variables and command lines are
never reported.

Diagnostics come from rustc's JSON diagnostics in the failed action's stderr.
`status` is `parsed` when diagnostics were read, `none` when the stderr held
none, `unavailable` when the stderr is remote or unreadable, and `unknown` for
non-Rust actions. A Rust action whose diagnostics include a `linking with`
error is reported as `linking`. Pass a rustc JSON error format through Bazel
to get diagnostics, for example
`-- --@rules_rust//rust/settings:error_format=json`.

If Bazel's event file ends early or cannot be read, dx still returns Bazel's
code and reports a `build_events_incomplete` notice.

Exit codes: `0` success. `2` usage or scope errors. `1` operational
failures. Bazel failures keep Bazel's code. A missing, truncated or invalid
`test.xml` artifact fails the run as incomplete, even when Bazel passes.

```sh
bazel run @rules_dx//:dx -- build //cli/...
bazel run @rules_dx//:dx -- test --here
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

## `dx coverage`

```text
dx coverage [--here] [--min-coverage <percent>] [--report lcov=<path>] [scope...] [-- bazel-options...]
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

Exit codes: `0` success. `2` usage or scope errors. `1` operational
failures, coverage below minimum, or incomplete coverage. Bazel failures
keep Bazel's code. Coverage is incomplete when Bazel reports a
`coverage.dat` artifact that is missing or invalid, or when no coverage
artifacts are reported. Incomplete coverage fails the run, blocks
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
