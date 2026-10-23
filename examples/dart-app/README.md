# Dart ledger example

A multi-package Dart app. It has a `ledger` CLI, a `ledger` library that parses CSV and sums spending through `collection`, a generated `Category` enum and a packaged `entries.csv` runfile.

| Package | Role |
| --- | --- |
| `ledger` | Entry model, CSV reader, totals, generated `Category`, `entries.csv` |
| `cli` | `ledger` binary with `totals`, `list`, `total CATEGORY` and `categories` |
| `diagnostics` | Sources that fail to compile, analyze or format |

```sh
bazel build //examples/dart-app/...
bazel test //examples/dart-app/...
bazel run //examples/dart-app/cli:ledger -- totals
bazel run //examples/dart-app/cli:ledger -- total food
bazel run //examples/dart-app/cli:ledger -- --file path/to/ledger.csv list
```

Without `--file` or `LEDGER_FILE`, `ledger` reads the packaged `entries.csv` through `package:runfiles`. Relative `--file` paths resolve against the directory you ran `bazel run` from. `--version` prints the version set with `defines` in `cli/BUILD.bazel`.

Exit codes: `0` ok, `2` usage, `3` unreadable or malformed ledger, `4` unknown category. `exit_codes_test` runs the binary through each code.

`args`, `collection` and `test` come from the locked `@dx_dart_deps` hub. `categories_g` runs `tool/categories_gen.dart` over `lib/src/categories.dart` to write `categories.g.dart`. Tests use `package:test`.

`analysis_options.yaml` sets a 100 column page width, strict casts, inference and raw types, and extra lint rules. The `dart_format_config` and `dart_analyze_config` hints point `dart format` and `dart analyze` at it. Turn both tools on with a `dart` family:

```python
quality_family(
    name = "dart_family",
    family_id = "dart",
    format = ["dart_format"],
    lint = ["dart_analyze"],
)
```

`diagnostics_test` compiles, formats and analyzes files with a syntax error, a type error, an undeclared package, lint findings and bad formatting, and expects each to fail.
