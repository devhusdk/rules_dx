# Java invoice example

A multi-package Java app. It has an invoice CLI, a `billing` library that reads JSON through `gson` and groups sums through `guava`, a generated `BuildInfo` class and a packaged `invoices.json` resource.

| Package | Role |
| --- | --- |
| `billing` | Invoice model, JSON reader, totals, generated `BuildInfo`, `invoices.json` |
| `cli` | `invoice` binary with `list`, `total`, `customers`, `schema` and `show ID` |

```sh
bazel build //examples/java-app/...
bazel test //examples/java-app/...
bazel run //examples/java-app/cli:invoice -- total
bazel run //examples/java-app/cli:invoice -- show INV-1001
bazel run //examples/java-app/cli:invoice -- --file path/to/invoices.json list
```

Without `--file` or `INVOICE_FILE`, `invoice` reads the `invoices.json` packaged in the `billing` jar, so the binary runs from any directory. Relative `--file` paths resolve against the directory you ran `bazel run` from. Quantities and prices must be whole numbers from `0` to `1000000`.

Exit codes: `0` ok, `2` usage, `3` unreadable or malformed invoice file, `4` unknown invoice. `exit_codes_test` runs the binary through each code.

`gson` and `guava` come from the locked `@maven` hub. `classpath_test` checks that the app classes load from the `billing` jar and the library classes from their own jars. Tests use JUnit 4. Sources build at the repository Java language level with `-Xlint:all -Werror`.

`format_test` checks the sources against the pinned `google-java-format`. `diagnostics_test` compiles, lints and formats files with a syntax error, a type error, a compiler warning, an unused import and bad formatting, and expects each to fail It also compiles the `billing` sources: they fail without `gson` or `guava` on the classpath and pass with both.
