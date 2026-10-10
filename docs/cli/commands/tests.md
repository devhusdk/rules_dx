# `dx tests`

```text
dx tests [scope ...] [-- bazel-options ...]
dx tests --cases [scope ...] [-- bazel-options ...]
dx tests --configured [scope ...] [-- bazel-options ...]
dx tests --here [-- bazel-options ...]
```

Lists the configured test targets for a scope without running them. The
inventory reads Bazel facts through the `tests()` query function, so a
custom test rule counts when Bazel marks it a test, even when its name
does not end in `_test`. An explicit label that is not a test, and a
pattern that matches no tests, fail instead of reporting an empty suite.

A bare run lists `tests(//...)`. Label and pattern scopes pass through;
workspace-relative files and directories resolve to their owning tests
first. `--here` (`--cwd` alias) lists the current directory tree instead
and never combines with explicit scopes. Trailing `--` options reach every
Bazel invocation behind the command, including the inventory query.

The default inventory is unconfigured (`bazel query`). `--configured`
reads configured targets through `bazel cquery` instead. File ownership
resolution stays unconfigured in both modes. `--dry-run` prints the
planned queries without running them.

`--cases` enumerates test cases for Rust targets (`rust_test` and the
`dx_wrap` forwarder) by building each test and running it with `--list`.
Listing never runs test bodies, but it builds the test and executes its
initialization code; treat the target as untrusted code. Every other
framework reports `unsupported_framework` with its rule kind, never an
empty suite. Case output is sorted and deterministic, capped at 2000 cases
per target and 5000 total; a cap reports `cases_truncated` instead of
silent success. Inventory stays useful without case enumeration.

Output: `--output text|json`. Text prints one label per line, or one
`label: case` line per case with `--cases`; problems go to stderr. JSON
starts with `command_started`, adds one `status` event per label
(`detail` is the label, `hint` names the scope and the query verb), one
`status` event per case with `--cases` (`detail` is the case, `hint` is
the label), one `error` event per problem, and ends with
`command_finished`.

Exit codes: `0` success, `2` usage errors including an external scope, a
relative label, or an unreadable path, `1` operational failures including
`no_tests`, `bazel_failed`, `unsupported_framework`, `no_cases`,
`case_failed`, or `cases_truncated`. A partial match still exits `1` when
any scope contributed no tests.

```sh
bazel run @rules_dx//:dx -- tests //cli/test_scratch/...
bazel run @rules_dx//:dx -- tests --cases //cli/test_scratch:dx_test_scratch_test_upstream
bazel run @rules_dx//:dx -- tests --cases //cli/test_scratch/... --output=json
bazel run @rules_dx//:dx -- tests --configured //cli/test_scratch/... -- --config=ci
```
