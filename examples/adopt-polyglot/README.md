# Adopt polyglot example

Mixed tree with a Python package, a Rust crate, a JavaScript package, a TypeScript package,
and a C++ library with a handwritten binary in its own child package.

`dx init` writes nothing inside this repository. For your own repo, see
[Adopting rules_dx without Bazel experience](../../docs/adopting-without-bazel.md).

Write BUILD files and format:

```sh
tools/dx check --apply //examples/adopt-polyglot/...
```

Build:

```sh
tools/dx build //examples/adopt-polyglot/...
```

Test:

```sh
tools/dx test //examples/adopt-polyglot/...
```

Five tests are declared here: `rectangles_test`, `widgets_test`, `totals_test`,
`native_test` and `consumer_test`. The TypeScript wrapper emits its own
typecheck tests, so `dx test` runs more targets than the example declares.

Check:

```sh
tools/dx check //examples/adopt-polyglot/...
```
