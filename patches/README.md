# Patches

Bazel applies `single_version_override` only in the root module, and its
patches must live in the root repository. `dx init --apply` and
`dx new --apply` write them into your tree:

```text
third_party/rules_dx/rules_dx.MODULE.bazel
third_party/rules_dx/<module>.<file>.patch
third_party/rules_dx/BUILD.bazel
third_party/rules_dx/.gitattributes
```

When `MODULE.bazel` has `bazel_dep(name = "rules_dx")`, dx adds this line after it:

```python
include("//third_party/rules_dx:rules_dx.MODULE.bazel")
```

dx owns `third_party/rules_dx/` and rewrites it on every `dx init --apply`. Do
not edit it. Rerun `tools/dx init --apply` after you move the `rules_dx` pin.
`MODULE.bazel.lock` changes with it. The overrides hold these modules at the
versions below across your whole module graph.

If your `MODULE.bazel` already overrides one of these modules, dx writes the
files but leaves the `include()` out and says so on stderr. Without a
`rules_dx` `bazel_dep`, dx writes the files and leaves `MODULE.bazel` alone.

| Module | Version | Patches | Why |
| --- | --- | --- | --- |
| `rules_rust` | 0.74.0 | `lld_link.patch` | Rust on Windows |
| `rules_rust_wasm_bindgen` | 0.74.0 | `no_wasm_coverage.patch` | wasm targets under coverage |
| `rules_android` | 0.6.6 | `rules_android.patch` | Android resources and App Bundles |
| `rules_dotnet` | 0.22.1 | `reproducible.patch` | .NET SDK from the repository cache |
| `rules_ruby` | 0.28.0 | `reproducible.patch` | Ruby toolchain from the repository cache |
| `aspect_rules_py` | 2.0.0-alpha.6 | `site_packages.patch`, `venv_interpreter.patch` | Python on Windows |

## rules_d, rules_elixir, rules_erlang, rules_fortran, rules_gleam, rules_haskell, rules_ocaml

No action. `rules_dx` applies these patches in its own repository rules.
