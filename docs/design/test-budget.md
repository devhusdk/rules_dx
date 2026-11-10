# Test budget design

Every test call follows these rules. `//tools/ci:test_budget_test` checks them.

## Budget

- Every test call sets `size`.
- The budget is `size = "small"` with no `timeout`.
- No test sets `flaky = True`.
- `test --test_verbose_timeout_warnings` in `.dx/bazelrc/correctness.bazelrc` warns when a test runs far under or over its declared size.

## Kinds

- The guard scans `@dx_test_tags_sources//:sources.json` as a declared input.
- It uses the test call detection of `//tools/ci:test_tags_test`. Both read `tools/ci/test_calls.rs`.
- It counts a test call that passes `name`. It skips `test_suite`.

| Kind | Counts |
| --- | --- |
| `no-size` | A test call with no `size` and no `**` argument. |
| `size-medium`, `size-large`, `size-enormous` | A test call with that `size`. |
| `small-timeout` | A test call with `size = "small"` and a `timeout`. |
| `timeout-long`, `timeout-eternal` | A test call with that `timeout`. |
| `flaky` | A test call with `flaky = True`. |

## Allowlist

- `tools/ci/test_budget_allowlist.txt` holds one line per file and kind: `<path> <kind> <count>`. Lines are sorted.
- The guard fails when a file has more hits than its line allows.
- The guard fails when a line allows more than the file has, or names a file with none.
- The allowlist only shrinks. No change adds a line or raises a count.

## Nested tests

- `tools/testing/nested_smoke.txt` holds one sorted line per nested test: `<label> <seconds>`.
- `<seconds>` is the cold time measured on Linux x86_64. A test with no measurement lists its declared timeout.
- A nested test is a test call in a BUILD file whose `tags` hold `resources:nested_bazel`, as a string or through a constant.
- A call of a macro that declares such a test is a nested test. A macro that declares one under an `if` counts when the call sets a name of the condition to `True`.
- A target whose name ends in `_upstream` is skipped.
- The guard fails on a nested test without a line and on a line without a nested test.
- The list holds at most 8 lines. The cap only goes down.
