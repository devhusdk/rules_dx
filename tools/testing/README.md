# tools/testing

## Rule-set fixtures

Each rule-set module `<lang>/` adds:

- `<lang>/tests/fixtures/plain/` with a `MODULE.bazel`, one `<lang>_binary` and one `<lang>_test`.
- `<lang>/<lang>_fixture_budget.json` with the keys `modules`, `lock_bytes`, `lock_extensions` and `external_repos`.
- A budget test that calls `dx_nested::plain_fixture::check_fixture` with the files from `plain_fixture::fixture_files`, and a case that calls `plain_fixture::measure_case`.
- One `nested_bazel_test` shared by php, lua and qt, with `cache`, `cache_locks` and `network = False`; each language is a case on one server, so the nested count stays 5 of 8.

`check_plain_consumer` expects `local_path_override` at `../../..`; a module whose root is not `<lang>/` adjusts the fixture path, not the checker. `count_modules` skips the roots named `consumer` and `rules_dx`, so the fixture's module is named `consumer`. The `local_config_cc` assertion waits on #12601.
