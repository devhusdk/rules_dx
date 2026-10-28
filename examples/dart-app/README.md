# Dart inventory example

A multi-package Dart app. It has an `inventory` CLI, a ledger package that parses a CSV file through `package:collection`, and a model package with a generated `schema.g.dart`.

| Package | Role |
| --- | --- |
| `gen` | `gen_schema` tool that writes `schema.g.dart` from `schema.json` |
| `model` | `Item` class and the generated schema |
| `ledger` | CSV parser and totals, `items.csv` read through runfiles |
| `cli` | `inventory` binary and its exit-code test |
| `failures` | Manual targets that fail on purpose |

```sh
bazel build //examples/dart-app/...
bazel test //examples/dart-app/...
bazel run //examples/dart-app/cli:inventory
bazel run //examples/dart-app/cli:inventory -- get cable
```

`inventory` prints each item, the total in cents and the schema version. `get NAME` prints one item.

Exit codes: `0` ok, `2` usage, `4` unknown item. `exit_codes_test` runs the binary through each code.

A `genrule` in `model` runs `gen_schema` as a tool, so Bazel builds the tool for the execution platform. The app uses the `dx_dart_deps` hub from `third_party/dart/pubspec.lock`. `ledger_test` covers malformed input.

`analysis_options.yaml` turns on `strict-casts`, `strict-raw-types` and two lints. A `dart_config` binds it to every target through `aspect_hints`.

The targets in `failures` are tagged `manual`. They have a type error, an import of a package that is not in `deps`, and an import of a file that does not exist. Each one fails with a compiler or dependency message.

rules_dart 0.8.0 ships no Windows arm64 SDK, so the targets are incompatible there.
