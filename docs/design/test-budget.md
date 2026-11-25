# Test budget design

Every test call follows these rules.

## Budget

- Every test call sets `size`.
- The budget is `size = "small"` with no `timeout`.
- No test sets `flaky = True`.
- `test --test_verbose_timeout_warnings` in `.dx/bazelrc/correctness.bazelrc` warns when a test runs far under or over its declared size.
