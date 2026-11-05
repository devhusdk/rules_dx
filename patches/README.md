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
| `rules_rust` | 0.75.0 | `clippy_metadata_inputs.patch`, `pipelined_rmeta_path.patch`, `crate_universe_root_lookup.patch`, `exec_cc_toolchain_env.patch` | Rust on Windows (MSVC stdlib link flags, bazelbuild/rules_rust#4376); clippy reads dependency `.rmeta`; the full rustc action writes its own `.rmeta` so it does not hit the read-only one from the metadata action (bazelbuild/rules_rust#3383, #3662; drop when a rules_rust release has it); `crate_universe` looks up the root `MODULE.bazel` only in the root module, so a consumer without a root `BUILD.bazel` can use `dx_crates` (#6144); build scripts get `HOST_CC`/`HOST_CXX`/`HOST_AR`/`HOST_CFLAGS`/`HOST_CXXFLAGS` from the exec cc toolchain and `TARGET_*` from the target one |
| `rules_android` | 0.8.0 | `bundle_keystore_properties.patch` | App Bundles with key rotation: write the oldest-signer properties file instead of reading the removed `_bundle_keystore_properties` attribute (#3739) |
| `rules_cuda` | 0.4.0-beta1 | `redist_cccl_key.patch` | CUDA 13.3+ redist manifests key CCCL as `cccl` instead of `cuda_cccl`; the helper accepts both |
| `protobuf` | 36.2 | `no_crate_universe.patch` | Drops protobuf's own `crate_universe` use so it does not clash with `dx_crates` |
| `rules_julia` | 0.8.0 | `package_name.patch`, `portable_cpu_target.patch` | Julia `package_name` attribute for the wrapper targets (periareon/rules_julia#42); package precompiles target portable CPUs so a cache built on one host loads on another (#6580) |
| `rules_dart` | 0.8.1 | `coverage.patch` | `dart_test` writes LCOV under `bazel coverage`: the test launcher passes `--coverage` to `package:test` and converts the report |
| `rules_py` | 2.0.0-alpha.11 | `py_cc_toolchain_platform.patch` | `py_cc_toolchain` picks the cc toolchain for the target platform |
| `rules_py` | 2.0.0-alpha.11 | `native_sdist_toolchain_flags.patch` | Native sdist builds get the cc toolchain flags (aspect-build/rules_py#1664) |
| `rules_py` | 2.0.0-alpha.11 | `pep517_msvc.patch` | PEP 517 builds find the MSVC toolchain on Windows (aspect-build/rules_py#1662) |
| `aspect_rules_ts` | 3.10.1 | `typecheck_root_dir.patch` | TypeScript typecheck root dir (aspect-build/rules_ts#1001) |
| `avro-cpp` | 1.12.0.bcr.1 | `iterator.patch` | Missing std includes (`<iterator>`, `<memory>`, `<vector>`) with libc++, added to the headers that use them; `Exception.hh` includes `fmt/format.h` for fmt 12 |
| `fmt` | 12.2.0.bcr.1 | none | Raises avro-cpp's fmt to a release with the libc++ include fix |
| `gcc_toolchain` | 0.13.0 | `sanitizer_spec.patch` | Ships `libsanitizer.spec` so GCC `-fsanitize=` links in the sandbox |
| `gcc_toolchain` | 0.13.0 | `runtime_libs.patch` | Exposes the GCC runtime libraries (libstdc++, libgcc) to link actions |
| `gcc_toolchain` | 0.13.0 | `target_identity_config.patch` | `cc_toolchain_config` takes `target_libc` and `target_system_name` and reports them instead of `local` |
| `gcc_toolchain` | 0.13.0 | `target_identity_defs.patch` | Repository rule passes `target_libc` and `<arch>-linux-gnu` into the generated toolchain |
| `gcc_toolchain` | 0.13.0 | `target_identity_extension.patch` | Module extension forwards `target_libc` and the target system name to the repository rule |
| `windows_support` | 0.4.1 | `host_tools.patch` | Opt-in `host_tools` tag keeps the MSVC compiler tools and Windows SDK `bin` (hermeticbuild/windows_support#19) |
| `windows_support` | 0.4.1 | `rules_dx_hosts.patch` | `host_tools` is on by default under `rules_dx`, so `@dx_msvc` has `cl.exe` |

The trade-off: a `single_version_override` pins the module for the whole
graph. If another dependency of yours needs a newer version of one of these
modules, Bazel still resolves the version above. Move the `rules_dx` pin
instead, or drop the `include()` and own the override yourself.

## elvis, rules_d, rules_dotnet, rules_elixir, rules_erlang, rules_fortran, rules_haskell, rules_ocaml, rules_powershell, rules_ruby

No action. `rules_dx` applies these patches in its own repository rules.

## ring (crate annotation)

`MODULE.bazel` patches ring 0.17.14's `build.rs` so it accepts clang-cl on
`aarch64-pc-windows-msvc` instead of looking for plain `clang` (#6337). It is
applied to rules_dx's own `dx_crates` and is not exported to consumers.

## gomod-parser (crate annotation)

`MODULE.bazel` patches gomod-parser 0.5.3 so a require version followed
directly by `//` parses (#4008, baz-scm/gomod-parser#52). It is applied to
rules_dx's own `dx_crates` and needs nothing from consumers. Remove it once a
gomod-parser release after 0.6.0 carries the fix.
