# C++ pantry example

A multi-package C++17 app. It has a menu report binary, a templated `group_sum`, a `fmt` dependency, a generated `build_config.h` header and a `menu.csv` resource found through runfiles.

| Package | Role |
| --- | --- |
| `core` | Menu parser, `group_sum` template, generated `build_config.h`, `menu.csv` |
| `app` | `pantry` binary and report formatter |

```sh
bazel build //examples/cpp-app/...
bazel test //examples/cpp-app/...
bazel run //examples/cpp-app/app:pantry
bazel run //examples/cpp-app/app:pantry -- path/to/menu.csv
```

Without a path, `pantry` reads the packaged `menu.csv` through runfiles, so the binary runs from any directory. A relative path resolves against the directory you ran `bazel run` from.

Exit codes: `0` ok, `3` runfiles unavailable, `4` menu file missing or malformed. `exit_codes_test` runs the binary through each code.

`fmt` is a pinned `http_archive` declared in `MODULE.bazel`. Tests use GoogleTest. `pantry_output_test` runs the binary and compares its report. `format_test` checks the sources against the repository `.clang-format`.

`diagnostics_test` compiles and lints files with a syntax error, a type error and a lint finding, and expects each to fail.

Another workspace builds, tests, runs and copies both apps with `bazel test //cc/tests/consumer:app_consumer_test`.
