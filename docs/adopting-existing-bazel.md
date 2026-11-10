# Adopting rules_dx In An Existing Bazel Repo

1. Commit a clean tree and run `bazel test //...`. It must pass.

2. Add the dep to `MODULE.bazel`:

   ```python
   bazel_dep(name = "rules_dx", version = "...")
   ```

3. Preview, then write the files:

   ```sh
   bazel run @rules_dx//:dx -- init
   bazel run @rules_dx//:dx -- init --apply
   ```

   Commit `MODULE.bazel.lock`. See [Init](cli/commands/init.md).

4. Run `dx` on existing targets:

   ```sh
   tools/dx build //pkg/...
   ```

6. Move one package. Change the `load()`.

   Rust keeps its names:

   ```starlark
   load("@rules_dx//rust:defs.bzl", "rust_library", "rust_test")
   ```

   Python changes names:

   ```starlark
   load("@rules_dx//python:defs.bzl", "python_library", "python_test")
   ```

   Other languages: see [Rule names](cli/commands/README.md#rule-names).

7. Check the package:

   ```sh
   tools/dx check //pkg/...
   ```

8. Test the package:

   ```sh
   tools/dx test //pkg/...
   ```

9. Move the next package.

## Roll back

- Package: restore its `load()` lines.
- Version: see [Upgrade](cli/commands/upgrade.md).
- Remove `rules_dx`: delete the `bazel_dep`.
