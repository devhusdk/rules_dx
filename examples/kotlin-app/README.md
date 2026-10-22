# Kotlin gradebook example

A multi-package Kotlin app. It has a gradebook CLI, a `grades` library that reads JSON through `gson` and groups students through `guava`, a generated `BuildInfo` object and a packaged `grades.json` resource.

| Package | Role |
| --- | --- |
| `grades` | Student model, JSON reader, ranking, generated `BuildInfo`, `grades.json` |
| `cli` | `grades` binary with `list`, `ranks`, `letters`, `schema` and `show NAME` |

```sh
bazel build //examples/kotlin-app/...
bazel test //examples/kotlin-app/...
bazel run //examples/kotlin-app/cli:grades -- ranks
bazel run //examples/kotlin-app/cli:grades -- show ada
bazel run //examples/kotlin-app/cli:grades -- --file path/to/grades.json list
```

Without `--file` or `GRADES_FILE`, `grades` reads the `grades.json` packaged in the `grades` jar, so the binary runs from any directory. Relative `--file` paths resolve against the directory you ran `bazel run` from. Scores must be whole numbers from `0` to `100`, and every student needs at least one.

Exit codes: `0` ok, `2` usage, `3` unreadable or malformed grade file, `4` unknown student. `exit_codes_test` runs the binary through each code.

`gson` and `guava` come from the locked `@maven` hub. `classpath_test` checks that the app classes load from the `grades` jar and the library classes from their own jars. Tests use JUnit 4. Kotlin warnings are errors.

`format_test` checks the sources against the pinned `ktfmt` in Kotlin-lang style. `diagnostics_test` formats and lints files with a syntax error, a wildcard import and bad formatting, and expects each to fail.
