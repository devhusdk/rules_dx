# Contributing

`rules_dx` is a Bazel developer platform. `dx` runs through Bazel.

```sh
bazel build //...
bazel test //...
```

Read [AGENTS.md](AGENTS.md) before changing code.

- Treat warnings as errors.
- Run the formatter, linter, and focused tests for your change.
- Update tests and user docs when behavior changes.
- Do not edit generated files. Use the generator.
- Library crates return typed errors. Only binaries map exit codes.
- The CI pin step reads `MODULE.bazel` as text. A multiline call, reordered
  arguments and comments are fine. A commented-out pin never counts. The
  version must be a quoted string. One declaration only. A second one fails.
  An unrelated dependency at the same version never passes. A missing,
  ambiguous or mismatched pin fails with the line, the declared version and
  the expected one.
- Each consumer check runs one command: `lint` runs `dx lint --check //...`,
  `typecheck` runs `dx typecheck --check //...`, `format` runs
  `dx format --check //...`, `generate` runs `dx generate --check //...`,
  `security-audit` runs `dx security //...`, `license-audit` runs
  `dx license //...`, `test` runs `dx test //...`, `build` runs
  `dx build //...`, `coverage` runs `dx coverage //...` plus
  `--min-coverage <percent>` when `min_coverage` is set. `--check` applies
  to the four quality checks only.
- A rejected workflow input fails in the `validate` job before any check
  starts, and `dx-ci (aggregate)` names the failure. An unknown, repeated
  or empty `disabled_checks` ID fails. Malformed JSON, a non-array value,
  an empty array, a repeated, unknown or spaced `platforms` label fails.
  `sequential` scheduling fails, `true` code scanning opt-in fails, and a
  malformed `min_coverage` value fails. `dx-ci (aggregate)` also fails when
  a check fails, is cancelled, never reports, or is skipped without being
  listed in `disabled_checks`. The evidence workflow validates inputs the
  same way; each cell uploads its log as the `evidence-<platform>` artifact
  and writes the tail to the job summary.
- Every Rust crate root denies `clippy::expect_used`, `clippy::unwrap_used`,
  `clippy::unreachable` and `clippy::todo` outside `cfg(test)`, and
  `//:clippy_policy_test` names each one.
