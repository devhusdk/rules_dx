# Adopt polyglot example

This example shows you a mixed repository that dx builds without hand-written Bazel files. It holds a Python package, a Rust crate, a JavaScript package, a TypeScript package, and a C++ library with a binary in its own child package.

Write the `BUILD.bazel` files, which declare what Bazel builds, and format the sources:

```sh
tools/dx check --apply //examples/adopt-polyglot/...
```

Then build, test and check it:

```sh
tools/dx build //examples/adopt-polyglot/...
tools/dx test //examples/adopt-polyglot/...
tools/dx check //examples/adopt-polyglot/...
```

To do the same in your own repository, follow [Adopting rules_dx without Bazel experience](../../docs/adopting-without-bazel.md).
