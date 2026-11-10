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

   `init` adds `include("//third_party/rules_dx:rules_dx.MODULE.bazel")` to `MODULE.bazel` and writes `third_party/rules_dx/`. Do not edit that directory. Commit `MODULE.bazel.lock`. See [Init](cli/commands/init.md).

4. If your `MODULE.bazel` already overrides `rules_rust`, `rules_android`, `rules_dotnet`, `rules_swift`, `aspect_rules_py`, `aspect_rules_ts` or `aspect_rules_jest`, `init` skips the `include()` and says so on stderr. Merge the override from `third_party/rules_dx/rules_dx.MODULE.bazel`, or remove yours.

5. Run `dx` on existing targets:

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
- Remove `rules_dx`: delete the `bazel_dep`, the `include()` line and `third_party/rules_dx/`. Restore your own overrides.
