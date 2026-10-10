# Build, Test, And Coverage Commands

```sh
bazel run @rules_dx//:dx -- build //...
bazel run @rules_dx//:dx -- test //...
bazel run @rules_dx//:dx -- coverage //...
```

No scope means `//...`. Pass a label, pattern, file, or dir to narrow it.
Use `--here` for the current dir tree. File scopes resolve to the owning
targets. Args after `--` go to Bazel unchanged.

## `dx build` And `dx test`

```text
dx build [--here] [--debug|--release] [scope...] [-- bazel-options...]
dx test [--here] [--debug|--release] [--strict-evidence] [--run-output <dir>] [--report junit=<path>] [scope...] [-- bazel-options...]
```

Builds or tests the scope. `--debug` uses the `dx_debug` profile,
`--release` uses `dx_release`. No flag uses `dx_dev`. The two flags conflict.
Extra `--config` values pass through after the dx profile in their original
order. Only a conflicting dx profile is rejected. `--strict-evidence` fails
the run when collected test evidence is incomplete instead of tolerating gaps.

Output: `--output text|json`. Reports: `dx test` writes
`--report junit=<path>` JUnit reports. Repeat the flag for more files. Use `-`
for stdout. A relative path anchors at the workspace root; an absolute one writes
as given. Colliding destinations fail before anything runs, parents must exist,
and a failed write keeps the previous file bytes. `dx build` has no report
format. `dx test --output=json` adds one
`test_outcome` event per test result, with its Bazel status, run, shard,
attempt, cached state, duration, case counts, artifact counts, and whether its
evidence is complete. `--run-output <dir>` copies every reported test output
into a unique child directory and writes `manifest.json` there. The manifest
lists target, run, shard, attempt, name, and retained path for each artifact.
Missing artifacts stay explicit. `--output=json` stays NDJSON on stdout and
adds one `report` event with format `run-output` for the manifest.

Exit codes: `0` success. `2` usage or scope errors. `1` operational
failures. Bazel failures keep Bazel's code. Missing or invalid `test.xml`
artifacts warn and pass when Bazel passes, other results exist, and at most
a quarter of the reported results are unusable. More than that fails as
incomplete. `--strict-evidence` fails any incomplete evidence instead.

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
reject `--test_arg`; run those cases with `dx bazel` instead.

Tests run once by default. Only tests marked flaky in their build
definition retry, up to three attempts. Diagnose one test with retries
through `dx test <label> -- --flaky_test_attempts=3`, or through
`bazel test --config=retry <label>`. A test that passes after retry is
reported as `passed_after_retry` with its attempt number.

## `dx run`

```text
dx run [--apply] [--debug|--release] <label...> [-- args...]
```

Builds the runnable targets in scope without launching them. Pass
`--apply` to launch the built targets in scope order. Explicit labels
and patterns run sequentially. File and directory scopes must resolve
to exactly one runnable target. Args after `--` go to the app, and only
reach it on an `--apply` launch. `dx run` works with `CI=true`. The
default check validates without launching. Pass `--apply` to launch a
noninteractive target in CI.

Output: `--output text|json`. `dx run` has no report format.

Exit codes: `0` success, `2` usage or scope errors, `1` no runnable target or
a launch failure. The app's own code is the exit code. A passing check
names the `--apply` command that launches.

```sh
bazel run @rules_dx//:dx -- run @rules_dx//:dx -- --help
bazel run @rules_dx//:dx -- run --apply @rules_dx//:dx -- --help
```

## `dx deploy`

```text
dx deploy [--apply] [--debug|--release] <label> [-- args...]
```

Builds one deployable target without publishing it. Pass `--apply` to run
the deployment. Takes exactly one label. No flag uses `dx_release` here.
Args after `--` go to the app, and only reach it on an `--apply` launch.

Output: `--output text`.

Exit codes: `0` success, `2` usage or scope errors, `1` a build or launch
failure. The target's own code is the exit code. A passing check names
the `--apply` command that publishes.

## `dx coverage`

```text
dx coverage [--here] [--min-coverage <percent>] [--strict-evidence] [--run-output <dir>] [--report lcov=<path>] [scope...] [-- bazel-options...]
```

Collects LCOV coverage over the scope. `--min-coverage` fails below that
percent. Without it, coverage collects without enforcing. `--strict-evidence`
is accepted and keeps scripts uniform; coverage already fails on incomplete
evidence with or without it.

Output: `--output text|json`. Reports: `--report lcov=<path>` writes
combined LCOV. Repeat the flag for more files. Use `-` for stdout. A relative
path anchors at the workspace root; an absolute one writes as given. Colliding
destinations fail before anything runs, parents must exist, and a failed write
keeps the previous file bytes.
`dx coverage --output=json` adds one `test_outcome` event per test result,
with its Bazel status, run, shard, attempt, cached state, duration, artifact
counts, and whether its evidence is complete. `--run-output <dir>` copies
every reported coverage output into a unique child directory and writes
`manifest.json` there. The manifest lists target, run, shard, attempt, name,
and retained path for each artifact. Missing artifacts stay explicit.
`--output=json` stays NDJSON on stdout and adds one `report` event with
format `run-output` for the manifest.

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
bazel run @rules_dx//:dx -- coverage --min-coverage 96 //...
```

Coverage ignores use `LCOV_EXCL_LINE` or `LCOV_EXCL_START` / `LCOV_EXCL_STOP`
with a short `reason:` and `issue:`. Bare ignores without a reason fail the
gate.
