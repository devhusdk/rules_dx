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

Exit codes: `0` ok, `3` runfiles unavailable, `4` menu file missing or malformed.

`fmt` is a pinned `http_archive` declared in `MODULE.bazel`. Tests use GoogleTest. `pantry_output_test` runs the binary and compares its report.
