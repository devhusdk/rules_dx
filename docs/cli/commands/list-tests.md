# `dx list-tests`

```text
dx list-tests [scope ...] [-- bazel-options ...]
dx list-tests --cases [scope ...] [-- bazel-options ...]
dx list-tests --configured [scope ...] [-- bazel-options ...]
dx list-tests --here [-- bazel-options ...]
```

Lists the configured test targets for a scope. It never builds or runs
them unless you pass `--cases`. Use `dx test` to run tests. The old
`dx tests` is rejected with exit `2`. The inventory reads Bazel facts
through the `tests()` query function, so a custom test rule counts when Bazel marks it a test, even when its name
does not end in `_test`. An explicit label that is not a test, and a
pattern that matches no tests, fail instead of reporting an empty suite.

A bare run lists `tests(//...)`. Label and pattern scopes pass through;
workspace-relative files and directories resolve to their owning tests
first. `--here` lists the current directory tree instead
and never combines with explicit scopes. Trailing `--` options reach every
Bazel invocation behind the command, including the inventory query.

The default inventory is unconfigured (`bazel query`). `--configured`
reads configured targets through `bazel cquery` instead. File ownership
resolution stays unconfigured in both modes.

`--cases` enumerates test cases for Rust targets (`rust_test` and the
`dx_wrap` forwarder) by building each test and running it with `--list`.
Listing never runs test bodies, but `--cases` builds the selected tests
and executes their binaries and initialization code; pass it only for
targets you trust. Every other
framework reports `unsupported_framework` with its rule kind, never an
empty suite. Case output is sorted and deterministic, with no cap. It ends with
a completeness line (`complete` or `incomplete`) on stderr, or a
`list-tests-summary` status event (`ok` or `warn`) with `--output json`. Inventory stays useful without case enumeration.

Output: `--output text|json`. Text prints one label per line, or one
`label: case` line per case with `--cases`; problems go to stderr. JSON
starts with `command_started`, adds one `status` event per label
(`detail` is the label, `hint` names the scope and the query verb), one
`status` event per case with `--cases` (`detail` is the case, `hint` is
the label), one `error` event per problem, and ends with
`command_finished`.

Exit codes: `0` success, `2` usage errors including an external scope, a
relative label, or an unreadable path, `1` operational failures including
`no_tests`, `bazel_failed`, `query_launch_failed`, `unsupported_framework`,
`no_cases`, or `case_failed`. A partial match still exits `1` when
any scope contributed no tests.

```sh
bazel run @rules_dx//:dx -- list-tests //cli/test_scratch/...
bazel run @rules_dx//:dx -- list-tests --cases //cli/test_scratch:dx_test_scratch_test_upstream
bazel run @rules_dx//:dx -- list-tests --cases //cli/test_scratch/... --output=json
bazel run @rules_dx//:dx -- list-tests --configured //cli/test_scratch/... -- --config=ci
```
