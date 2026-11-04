# Adopt polyglot example

Mixed tree with a Python package, a Rust crate, a JavaScript package, a TypeScript package,
and a C++ library with a handwritten binary in its own child package.

```sh
tools/dx generate --apply //examples/adopt-polyglot/...
tools/dx build //examples/adopt-polyglot/...
tools/dx test //examples/adopt-polyglot/...
```

`dx init` writes nothing inside this repository. For your own repo, see
[Adopting rules_dx without Bazel experience](../../docs/adopting-without-bazel.md).

All 4 tests pass (`shapes_test`, `widgets_test`, `totals_test`, `native_test`).
The TypeScript wrapper emits its own typecheck tests, so `dx test` runs more
targets than the example declares.
