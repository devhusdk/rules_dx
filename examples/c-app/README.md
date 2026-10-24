# C stock example

A multi-package pure C app. It has a stock CLI, a store library that reads JSON through `cJSON`, a generated `store_build.h` header and a packaged `stock.json`.

| Package | Role |
| --- | --- |
| `store` | JSON loader and totals, `extern "C"` header, generated `store_build.h` |
| `cli` | `stock` binary with `list`, `total` and `get NAME` |

```sh
bazel build //examples/c-app/...
bazel test //examples/c-app/...
bazel run //examples/c-app/cli:stock -- total
bazel run //examples/c-app/cli:stock -- get bolt-m6
bazel run //examples/c-app/cli:stock -- --file path/to/stock.json list
```

Without `--file` or `STOCK_FILE`, `stock` reads the packaged `stock.json` through runfiles, so the binary runs from any directory. Relative `--file` paths resolve against the directory you ran `bazel run` from. Quantities and prices must be whole numbers from `0` to `1000000`.

Exit codes: `0` ok, `2` usage, `3` unreadable or malformed stock file, `4` unknown item. `exit_codes_test` runs the binary through each code.

Sources build as C11 with `-std=c11`. `store_test` and `cli_test` are C programs and fail to compile as C++. `linkage_test` includes the same header from C++ and links the C library. `cJSON` is a pinned `http_archive` declared in `MODULE.bazel`. `format_test` checks the sources against the repository `.clang-format`.

`diagnostics_test` compiles and lints files with a syntax error, a type error and a lint finding, and expects each to fail.

Another workspace builds, tests, runs and copies both apps with `bazel test //cc/tests/consumer:app_consumer_test`.
