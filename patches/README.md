# Patches

`single_version_override` only works in the root module, and its patches must
live in the root repository. A module that depends on `rules_dx` copies each
patch it needs into its own tree and adds the override to its `MODULE.bazel`.

```sh
mkdir -p third_party/rules_dx
cp <rules_dx>/patches/<module>/<file>.patch third_party/rules_dx/<module>.<file>.patch
```

`third_party/rules_dx/BUILD.bazel`:

```python
exports_files(glob(["*.patch"]))
```

Keep the copies byte-identical and update them when you move the `rules_dx` pin.

## rules_rust

Required for Rust on Windows. Copy `rules_rust/lld_link.patch` to
`third_party/rules_dx/rules_rust.lld_link.patch`.

```python
single_version_override(
    module_name = "rules_rust",
    patch_strip = 1,
    patches = ["//third_party/rules_dx:rules_rust.lld_link.patch"],
    version = "0.74.0",
)
```

Upstream: not filed.

## aspect_rules_py

Required for Python on Windows. Copy both patches, in this order.

```python
single_version_override(
    module_name = "aspect_rules_py",
    patch_strip = 1,
    patches = [
        "//third_party/rules_dx:aspect_rules_py.site_packages.patch",
        "//third_party/rules_dx:aspect_rules_py.venv_interpreter.patch",
    ],
    version = "2.0.0-alpha.6",
)
```

Upstream: not filed.

## rules_android

Required for Android resources and App Bundles. Copy `rules_android/rules_android.patch` to
`third_party/rules_dx/rules_android.rules_android.patch`.

```python
single_version_override(
    module_name = "rules_android",
    patch_strip = 1,
    patches = ["//third_party/rules_dx:rules_android.rules_android.patch"],
    version = "0.6.6",
)
```

Upstream: not filed.

## rules_dotnet

Optional. Lets Bazel reuse the .NET SDK from the repository cache.

```python
single_version_override(
    module_name = "rules_dotnet",
    patch_strip = 1,
    patches = ["//third_party/rules_dx:rules_dotnet.reproducible.patch"],
    version = "0.22.1",
)
```

Upstream: not filed.

## rules_ruby

Optional. Lets Bazel reuse the Ruby toolchain from the repository cache.

```python
single_version_override(
    module_name = "rules_ruby",
    patch_strip = 1,
    patches = ["//third_party/rules_dx:rules_ruby.reproducible.patch"],
    version = "0.28.0",
)
```

Upstream: not filed.

## rules_d, rules_elixir, rules_erlang, rules_fortran, rules_gleam, rules_haskell, rules_ocaml

No action. `rules_dx` applies these patches in its own repository rules.
