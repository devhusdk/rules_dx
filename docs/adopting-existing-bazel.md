# Adopting rules_dx In An Existing Bazel Repo

This page helps you add rules_dx to a repo that already builds with Bazel, one
package at a time.

## Steps

1. Commit a clean tree. Then run your tests with plain Bazel. They must pass
   before you start.

   ```sh
   bazel test //...
   ```

   `//...` is a target pattern. It means every target in the repo. A target
   is one thing Bazel can build or test, such as a library or a test.

2. Install `dx` as in the [tutorial](tutorial.md#install).

3. Add rules_dx to `MODULE.bazel`. `MODULE.bazel` is the file at the repo root
   that names your Bazel module and the modules it depends on. Use the version
   that `dx --version` prints:

   ```python
   bazel_dep(name = "rules_dx", version = "...")
   ```

4. Preview what `dx init` would write, then write it:

   ```sh
   dx init
   dx init --apply
   ```

   `dx init --apply` keeps your `MODULE.bazel`. It ends by running
   `dx generate --apply`, which writes `BUILD.bazel` files. Add `--no-generate`
   to skip that step. Commit `MODULE.bazel.lock`, the file where Bazel records
   the exact versions it resolved.

5. Build your existing targets through `dx`:

   ```sh
   tools/dx build //pkg/...
   ```

   `tools/dx` is the launcher `dx init` wrote. `//pkg/...` means every target
   under the `pkg` directory.

6. Move one package. A package is a directory with a `BUILD.bazel` file.
   Change its `load()` lines, which say which `.bzl` file each rule comes from.
   The labels start with `@rules_dx//`. A label is a Bazel name for a file or
   target: `@repo//path:name`.

7. Check the package:

   ```sh
   tools/dx check //pkg/...
   ```

8. Test the package:

   ```sh
   tools/dx test //pkg/...
   ```

9. Move the next package.

## Example

Rust keeps its rule names. Only the load label changes:

```starlark
load("@rules_dx//rust:defs.bzl", "rust_library", "rust_test")
```

Python changes rule names. `py_library` becomes `python_library` and
`py_test` becomes `python_test`:

```starlark
load("@rules_dx//python:defs.bzl", "python_library", "python_test")
```

For other languages, look up the dx rule in
[Rule names](cli/commands/README.md#rule-names).

## Roll back

- One package: restore its `load()` lines.
- The rules_dx version: run `dx upgrade --to <version> --apply`
  ([`dx upgrade`](cli/commands/upgrade.md)).
- All of rules_dx: delete the `bazel_dep` line from `MODULE.bazel`.

## Reference

- [`dx init`](cli/commands/init.md): every file init writes and every flag.
