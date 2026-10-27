# `dx list-tests`

```text
dx list-tests [scope ...] [-- bazel-options ...]
dx list-tests --cases [scope ...] [-- bazel-options ...]
dx list-tests --configured [scope ...] [-- bazel-options ...]
dx list-tests --here [-- bazel-options ...]
dx list-tests --affected [-- bazel-options ...]
```

Lists the configured test targets for a scope. It never builds or runs
them unless you pass `--cases`. Use `dx test` to run tests. The inventory reads Bazel facts
through the `tests()` query function, so a custom test rule counts when Bazel marks it a test, even when its name
does not end in `_test`. An explicit label that is not a test, and a
pattern that matches no tests, fail instead of reporting an empty suite.

A bare run lists `tests(//...)`. Label and pattern scopes pass through.
A workspace-relative file resolves to the tests that own it or depend on its
owners; a test tagged `manual` is kept only when it owns the file.
Workspace-relative directories resolve to their owning tests first. `--here` lists the current directory tree instead
and never combines with explicit scopes. `--affected` lists only the tests your
git changes reach (see [Scope Defaults](scope-defaults.md)); with none it
prints `no affected targets` and exits `0`. It never combines with scopes or
`--here`. Trailing `--` options reach every
Bazel invocation behind the command, including the inventory query.

The default inventory is unconfigured (`bazel query`). `--configured`
reads configured targets through `bazel cquery` instead. File ownership
resolution stays unconfigured in both modes, so owners are found without
configuration. With `--configured`, a file scope keeps only the tests that
reach its owners in the configured graph (cquery `rdeps`). Such a scope can
drop tests or fail with `no_tests`.

`--cases` enumerates test cases for Rust targets (`rust_test` and the
`dx_wrap` forwarder) by building each test and running it with `--list`.
Listing never runs test bodies, but `--cases` builds the selected tests
and executes their binaries and initialization code; pass it only for
targets you trust. The `--list` run also passes
`--@rules_dx//config:clippy_output_diagnostics=true`
and `--@rules_dx//config:rustc_output_diagnostics=true`. Another value for one of them,
or for the matching `@rules_rust//rust/settings` flag, fails before Bazel starts with exit `2`.
Every other
framework reports `unsupported_framework` with its rule kind, never an
empty suite. Case output is sorted and deterministic, with no cap. It ends with
a completeness line (`complete` or `incomplete`) on stderr, or a
`list-tests-summary` status event (`ok` or `warn`) with `--output json`. Inventory stays useful without case enumeration.

Output: `--output text|json`. `dx list-tests` has no report format. Text prints one label per line, or one
`label: case` line per case with `--cases`; problems go to stderr. JSON
starts with `command_started`, adds one `status` event per label
(`detail` is the label, `hint` names the scope and the query verb), one
`status` event per case with `--cases` (`detail` is the case, `hint` is
the label), one `error` event per problem, and ends with
`command_finished`. Each `error` event carries `code`, `message`, `cause`,
`phase`, and `status` when a Bazel run exited (`bazel_failed`, `case_failed`).

Exit codes: `0` success, `2` usage errors including an external scope, a
relative label, an unreadable path, a missing path (`invalid_scope`), a file no target owns (`no_owner`), or a Bazel option `--cases` cannot pass to
`bazel run`, `1` operational failures including
`no_tests`, `bazel_failed`, `query_failed`, `query_launch_failed`, `launch_failed`,
`unsupported_framework`, `no_cases`, or `case_failed`. A partial match still exits `1` when
any scope contributed no tests.

```sh
bazel run @rules_dx//:dx -- list-tests //cli/test_scratch/...
bazel run @rules_dx//:dx -- list-tests --cases //cli/test_scratch:dx_test_scratch_test_upstream
bazel run @rules_dx//:dx -- list-tests --cases //cli/test_scratch:dx_test_scratch_test_upstream --output=json
bazel run @rules_dx//:dx -- list-tests --configured //cli/test_scratch/... -- --config=ci
```
