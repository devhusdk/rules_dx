# Python service example

A multi-package Python app. It has a `click` command line, a local JSON service, a generated `build_info` module, packaged item data, and a pinned `uv.lock`.

| Package | Role |
| --- | --- |
| `ledger/core` | Catalog, config, packaged `items.json`, generated `build_info.py` |
| `ledger/service` | JSON server on `127.0.0.1` |
| `ledger/cli` | `ledger` binary with `info`, `list`, `quote` and `serve` |

```sh
bazel build //python/tests/fixtures/python-service/...
bazel test //python/tests/fixtures/python-service/...
bazel run //python/tests/fixtures/python-service/ledger/cli:ledger -- list
bazel run //python/tests/fixtures/python-service/ledger/cli:ledger -- quote bolt-m6 --qty 4
bazel run //python/tests/fixtures/python-service/ledger/cli:ledger -- serve --port 8080
```

Config comes from `--config FILE` or `LEDGER_CONFIG`. Keys: `currency`, `tax_percent`, `items_path`.

Tests use both runners: `catalog_test`, `server_test`, `cli_test` and `broken_test` run pytest. `build_info_test` and `cli_unittest_test` run unittest. `build_info_test` sets `PIP_NO_INDEX` and `UV_OFFLINE`, and `click` comes from the locked `pypi` hub.

Coverage instrumentation and runfiles contents are checked in `//python/tests/analysis`.

Refresh the lock with `uv lock --directory python/tests/fixtures/python-service`.

```sh
bazel test //python/tests/consumer:python_init_adopt_test
```
