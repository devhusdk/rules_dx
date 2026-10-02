# Adopt polyglot example

Mixed tree with a Python package, a Rust crate, a JavaScript package, and a TypeScript package.

```sh
bazel run //cli/cli:dx -- init //examples/adopt-polyglot/...
bazel run //cli/cli:dx -- generate //examples/adopt-polyglot/...
bazel build //examples/adopt-polyglot/...
bazel test //examples/adopt-polyglot/...
```

All 4 tests pass (`shapes_test`, `widgets_test`, `totals_test`, `native_test`).
The TypeScript wrapper emits its own typecheck tests, so `bazel test` runs more
targets than the example declares.
