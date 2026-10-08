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
dx test [--here] [--debug|--release] [--report junit=<path>] [scope...] [-- bazel-options...]
```

Builds or tests the scope. `--debug` uses the `dx_debug` profile,
`--release` uses `dx_release`. No flag uses `dx_dev`. The two flags conflict.

Output: `--output text|json`. Reports: `dx test` writes
`--report junit=<path>` JUnit reports. Repeat the flag for more files. Use `-`
for stdout. `dx build` has no report format.

Exit codes: `0` success. `2` usage or scope errors. `1` operational
failures. Bazel failures keep Bazel's code. Missing or invalid `test.xml`
artifacts warn and pass when Bazel passes, other results exist, and at most
a quarter of the reported results are unusable. More than that fails as
incomplete.

```sh
bazel run @rules_dx//:dx -- build //cli/...
bazel run @rules_dx//:dx -- test --here
bazel run @rules_dx//:dx -- test //... -- --jobs=4
```

## `dx run`

```text
dx run [--apply] [--debug|--release] <label...> [-- args...]
```

Builds the runnable targets in scope without launching them. Pass
`--apply` to launch the built targets in scope order. Explicit labels
and patterns run sequentially. File and directory scopes must resolve
to exactly one runnable target. Args after `--` go to the app, and only
reach it on an `--apply` launch.

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
dx coverage [--here] [--min-coverage <percent>] [--report lcov=<path>] [scope...] [-- bazel-options...]
```

Collects LCOV coverage over the scope. `--min-coverage` fails below that
percent. Without it, coverage collects without enforcing.

Output: `--output text|json`. Reports: `--report lcov=<path>` writes
combined LCOV. Repeat the flag for more files. Use `-` for stdout.

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
