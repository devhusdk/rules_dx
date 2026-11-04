# Patches

Bazel applies `single_version_override` only in the root module, and its
patches must live in the root repository. `dx init --apply` writes
them into your tree:

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
| `rules_rust` | 0.75.0 | `lld_link.patch`, `msvc_stdlib_linkflags.patch`, `clippy_metadata_inputs.patch`, `pipelined_rmeta_path.patch` | Rust on Windows (lld-link, MSVC stdlib link flags); clippy reads dependency `.rmeta`; the full rustc action writes its own `.rmeta` so it does not hit the read-only one from the metadata action |
| `rules_julia` | 0.8.0 | `package_name.patch` | Julia `package_name` attribute for the wrapper targets |
| `aspect_rules_py` | 2.0.0-alpha.10 | `windows_venv.patch` | Python venvs on Windows |
| `aspect_rules_ts` | 3.10.1 | `typecheck_root_dir.patch` | TypeScript typecheck root dir |
| `aspect_rules_jest` | 0.26.0 | `windows_v8_coverage.patch` | Jest coverage on Windows (v8 provider) |
| `avro-cpp` | 1.12.0.bcr.1 | `iterator.patch` | Missing std includes with libc++ |
| `fmt` | 11.1.4 | `cstdlib.patch` | Missing `<cstdlib>` include with libc++ |
| `gcc_toolchain` | 0.13.0 | `sanitizer_spec.patch` | Ships `libsanitizer.spec` so GCC `-fsanitize=` links in the sandbox |
| `windows_support` | 0.4.1 | `host_tools.patch` | Opt-in `host_tools` tag keeps the MSVC compiler tools and Windows SDK `bin` (hermeticbuild/windows_support#19) |

The trade-off: a `single_version_override` pins the module for the whole
graph. If another dependency of yours needs a newer version of one of these
modules, Bazel still resolves the version above. Move the `rules_dx` pin
instead, or drop the `include()` and own the override yourself.

## elvis, rules_d, rules_dart, rules_dotnet, rules_elixir, rules_erlang, rules_fortran, rules_haskell, rules_ocaml, rules_powershell, rules_ruby

No action. `rules_dx` applies these patches in its own repository rules.
