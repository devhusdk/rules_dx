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
- `generation/result` and `quality/result` share a leaf name for different
  protos. The crates are already distinct (`generation_result` versus
  `quality_result`), so the short directory names stay.
- Every Rust crate root denies `clippy::expect_used`, `clippy::unwrap_used`,
  `clippy::unreachable` and `clippy::todo` outside `cfg(test)`, and
  `//:clippy_policy_test` names each one.
