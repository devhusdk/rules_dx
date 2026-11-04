# Adopt polyglot example

Mixed tree with a Python package, a Rust crate, a JavaScript package, a TypeScript package,
and a C++ library with a handwritten binary in its own child package.

`dx init` writes nothing inside this repository. For your own repo, see
[Adopting rules_dx without Bazel experience](../../docs/adopting-without-bazel.md).

Write BUILD files and format:

```sh
tools/dx check --apply //examples/adopt-polyglot/...
```

```text
Generation failed:
python: @rules_dx//examples/adopt-polyglot/pytools:shapes_test: ambiguous import "shapes" resolves to [...]
```

This step currently fails, see [#6654](https://github.com/devhusdk/rules_dx/issues/6654).
The BUILD files are already checked in, so the next steps still run.

Build:

```sh
tools/dx build //examples/adopt-polyglot/...
```

```text
{"event":"command_finished","exit_code":0, ...}
```

Test:

```sh
tools/dx test //examples/adopt-polyglot/...
```

```text
Executed 7 out of 7 tests: 7 tests pass.
```

Four of the tests are declared here (`shapes_test`, `widgets_test`, `totals_test`, `native_test`).
The TypeScript wrapper emits its own typecheck tests, so `dx test` runs more
targets than the example declares.

Check, which should be clean but hits the same failure until #6654 is fixed:

```sh
tools/dx check //examples/adopt-polyglot/...
```

```text
Generation failed:
python: @rules_dx//examples/adopt-polyglot/pytools:shapes_test: ambiguous import "shapes" resolves to [...]
```

These commands were run on linux.
