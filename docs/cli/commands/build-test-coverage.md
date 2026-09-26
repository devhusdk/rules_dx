# Build, Test, And Coverage Commands

```sh
bazel run //cli/cli:dx -- build //...
bazel run //cli/cli:dx -- test //...
bazel run //cli/cli:dx -- coverage //...
```

No scope means `//...`. Pass a label, pattern, file, or dir to narrow it.
Use `--here` for the current dir tree. File scopes resolve to the owning
targets. Args after `--` go to Bazel unchanged.

## `dx build` And `dx test`

```text
dx build [--here] [--debug|--release] [scope...] [-- bazel-options...]
dx test [--here] [--debug|--release] [scope...] [-- bazel-options...]
```

Builds or tests the scope. `--debug` uses the `dx_debug` profile,
`--release` uses `dx_release`. No flag uses `dx_dev`. The two flags conflict.

```sh
bazel run //cli/cli:dx -- build //cli/...
bazel run //cli/cli:dx -- test --here
bazel run //cli/cli:dx -- test //... -- --jobs=4
```

## `dx run`

```text
dx run [--debug|--release] <label...> [-- args...]
```

Builds and runs runnable targets in scope order. Explicit labels and patterns
run sequentially. File and directory scopes must resolve to exactly one
runnable target. Args after `--` go to the app.

```sh
bazel run //cli/cli:dx -- run //cli/cli:dx -- --help
```

## `dx deploy`

```text
dx deploy [--debug|--release] <label> [-- args...]
```

Builds and runs one deployable target. Takes exactly one label. No flag uses
`dx_release` here. Args after `--` go to the app.

## `dx coverage`

```text
dx coverage [--here] [--min-coverage <percent>] [scope...] [-- bazel-options...]
```

Collects LCOV coverage over the scope. `--min-coverage` fails below that
percent. Without it, coverage collects without enforcing.

```sh
bazel run //cli/cli:dx -- coverage //...
bazel run //cli/cli:dx -- coverage --min-coverage 96 //...
```

Coverage ignores use `LCOV_EXCL_LINE` or `LCOV_EXCL_START` / `LCOV_EXCL_STOP`
with a short `reason:` and `issue:`. Bare ignores without a reason fail the
gate.
