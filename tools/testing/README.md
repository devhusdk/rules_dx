# tools/testing

## Rule-set fixtures

Each rule-set module `<lang>/` adds:

- `<lang>/tests/fixtures/plain/` with a `MODULE.bazel`, one `<lang>_binary` and one `<lang>_test`.
- `<lang>/<lang>_fixture_budget.json` with the keys `modules`, `lock_bytes`, `lock_extensions` and `external_repos`.
- A budget test that calls `dx_nested::plain_consumer::check_plain_consumer` and `dx_nested::footprint::check_budget`.
