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

The Version column is generated from `MODULE.bazel`, and every patch it applies must appear in the table. Refresh it with `bazel run //patches:update_readme`.

| Module | Version | Patches | Why |
| --- | --- | --- | --- |
| `rules_rust` | 0.75.0 | `clippy_metadata_inputs.patch`, `pipelined_rmeta_path.patch`, `crate_universe_root_lookup.patch`, `exec_cc_toolchain_env.patch`, `msvc_pdb_output.patch`, `rust_lld_msvc_libpath.patch`, `crate_universe_lazy_host_tools.patch`, `cargo_toml_env_vars_flag.patch` | clippy reads dependency `.rmeta`; the full rustc action writes its own `.rmeta` so it does not hit the read-only one from the metadata action (bazelbuild/rules_rust#3383, #3662; drop when a rules_rust release has it); `crate_universe` looks up the root `MODULE.bazel` only in the root module, so a consumer without a root `BUILD.bazel` can use `dx_crates` (#6144); build scripts get `TARGET_*` from the target cc toolchain, and tool paths that are already absolute, including `C:/` drive-letter ones, are not prefixed with `${pwd}/`; when exec and target triples differ `HOST_CC`/`HOST_CXX`/`HOST_AR`/`HOST_CFLAGS`/`HOST_CXXFLAGS` come from the exec cc toolchain, otherwise `HOST_CC`/`HOST_CXX`/`HOST_AR` are the target tools and `HOST_CFLAGS`/`HOST_CXXFLAGS` are empty, since cc-rs then adds them on top of `CFLAGS` and two libc++ include dirs break ucrt's `errno.h` under clang-cl (#8681); Windows coverage compiles build-script C without ISO wide stdio so it links with rustc's profiler runtime (#8448); MSVC `bin` and `cdylib` crates always declare their `.pdb`, since rustc writes it whatever the strip level (rust-lang/rust#115120); with `linker_preference = "rust"`, MSVC links keep the CRT and Windows SDK `/LIBPATH:` dirs from the cc toolchain, so rust-lld finds `kernel32.lib` without a system Visual Studio, and keep its non-CRT `/DEFAULTLIB:` libs, so C++ from cc-rs finds `libc++.lib` (#8254, #8681; bazelbuild/rules_rust#4395, PR #4396; drop when a rules_rust release has it); `crate` hubs with a `lockfile` render from it without fetching `rust_host_tools` (cargo and rustc), unless a repin is requested or `CARGO_BAZEL_DIGEST_CHECK=1` is set, which CI preflight does (#8915); `--@rules_rust//cargo/settings:cargo_toml_env_vars=False` makes `cargo_toml_env_vars` write an empty env file, so `cargo_toml_variable_extractor` and the 55 crates it needs are never fetched (#8898; drop when a rules_rust release has the flag) |
| `rules_android` | 0.8.0 | `bundle_keystore_properties.patch` | App Bundles with key rotation: write the oldest-signer properties file instead of reading the removed `_bundle_keystore_properties` attribute (#3739) |
| `rules_kotlin` | 2.4.21 | `windows_coverage_test_suite.patch` | Windows `bazel coverage` of `kt_jvm_test`: the launcher no longer adds a second `-Dbazel.test_suite=<main class>` after the real one, which made `BazelTestRunner` run itself as the suite (#8793; bazel-contrib/rules_kotlin#1828, PR #1829; drop when a rules_kotlin release has it). `JACOCO_MAIN_CLASS` itself comes from `dx_jvm_test_coverage` in `//java/private:coverage.bzl`, since the Windows launcher ignores `jacoco_main_class` (bazelbuild/bazel#32479) |
| `rules_cuda` | 0.4.0-beta1 | `msvc_dlink_crt.patch`, `redist_cccl_key.patch`, `windows_arm64.patch` | `nvcc -dlink` on Windows gets the same `/MD` or `/MT` as the compile action, so rdc links do not mix LIBCMT with the dynamic CRT (#4704); CUDA 13.3+ redist manifests key CCCL as `cccl` instead of `cuda_cccl`; the helper accepts both (bazel-contrib/rules_cuda#523, merged; drop on the next release, #8365); windows-arm64 redists get their platform, `arm64` import libraries and an `nvcc_msvc` toolchain without the x86_64 pin (bazel-contrib/rules_cuda#528, vendored at `d56eade2`; drop with the others in #8365) |
| `protobuf` | 36.2 | `no_crate_universe.patch` | Drops protobuf's own `crate_universe` use so it does not clash with `dx_crates` |
| `rules_julia` | 0.8.0 | `package_name.patch`, `portable_cpu_target.patch`, `template_action_input.patch` | Julia `package_name` attribute for the wrapper targets (periareon/rules_julia#42); package precompiles target portable CPUs so a cache built on one host loads on another (#6580); the wrapper script is rendered by an action that reads the generated template as an input, so `julia_binary` builds without `--remote_download_outputs=all` (#9952) |
| `rules_julia` | 0.8.0 | `package_annotation_version.patch` | Package annotations are looked up by `name@version` before `name` |
| `rules_dart` | 0.8.1 | `coverage.patch` | `dart_test` writes LCOV under `bazel coverage`: the test launcher passes `--coverage` to `package:test` and converts the report |
| `rules_dart` | 0.8.1 | `external_pkgsrcs.patch` | Sources of packages from an external repository are staged into `.pkgsrcs`, which `copy_to_directory` otherwise drops (#9790) |
| `rules_swift` | 4.2.2 | `toolchain_sysroot_env.patch`, `non_root_toolchain.patch` | `swift.toolchain` takes a `sysroot` per platform and an `env` whose `{sysroot}` is that sysroot's root, so the exec Swift toolchain passes `-sdk` and finds host libraries in the locked Ubuntu sysroot (bazelbuild/rules_swift#1993); `swift.toolchain` works from a non-root module, so rules_dx declares `dx_swift` for consumers (bazelbuild/rules_swift#1996). Without this override, evaluating the `swift` extension fails with `unknown attribute sysroot`, so a consumer that owns its own `rules_swift` override must add both patches. Drop on release, #6528 |
| `rules_swift_package_manager` | 1.25.0 | `swift_executable.patch` | Dev only. `swift_deps.from_package` takes a `swift_executable` label that runs every `swift package` command, so the managed Swift fetches packages, the `@swift_package//:update` and `:resolve` tools run it, and the host needs no Swift or `xcrun`. A dev-only root usage reports its repos as dev deps. Send upstream, then drop (#8101) |
| `rules_python` | 2.4.6 | `single_import_lib.patch` | `py_extension` on Windows links one CPython import library: the stable ABI one for limited API modules, else the versioned one. It passed both as one linker argument (#7119; bazel-contrib/rules_python#4426, PR #4427; drop when a rules_python release has it, #9342) |
| `rules_python` | 2.4.6 | `lock_extension.patch` | The `python` extension is OS and architecture dependent and not reproducible, so a lockfile written on one host does not pin another |
| `rules_py` | 2.0.0-alpha.11 | `py_cc_toolchain_platform.patch` | `py_cc_toolchain` picks the cc toolchain for the target platform |
| `rules_py` | 2.0.0-alpha.11 | `native_sdist_toolchain_flags.patch` | Native sdist builds get the cc toolchain flags (aspect-build/rules_py#1664) |
| `rules_py` | 2.0.0-alpha.11 | `pep517_msvc.patch` | PEP 517 builds find the MSVC toolchain on Windows (aspect-build/rules_py#1662) |
| `rules_py` | 2.0.0-alpha.11 | `venv_bin_files.patch` | The runtime sets `venv_bin_files`, so `java_binary` analyzes on Windows; there the venv's `python.exe` and DLLs are file symlinks, so the launcher does not hit `CreateProcess failed` (aspect-build/rules_py#1670) |
| `rules_py` | 2.0.0-alpha.11 | `host_interpreter_windows_arm64.patch` | rules_py can see its `python_3_13_aarch64_pc_windows_msvc` interpreter, so sdist builds load on Windows ARM64 hosts (aspect-build/rules_py#1676) |
| `rules_py` | 2.0.0-alpha.11 | `external_repo_pytest_paths.patch` | `py_pytest_main` finds `.pytest_paths` for targets in an external repository under `../<repo>/<package>` |
| `hermetic_launcher` | 0.0.17 | `windows_arm64_toolchains.patch` | Registers template and finalizer toolchains for the prebuilt aarch64 Windows stubs (hermeticbuild/hermetic-launcher#94) |
| `aspect_rules_ts` | 3.10.1 | `typecheck_root_dir.patch` | TypeScript typecheck root dir (aspect-build/rules_ts#1001) |
| `avro-cpp` | 1.12.0.bcr.1 | `iterator.patch` | Missing std includes (`<iterator>`, `<memory>`, `<vector>`) with libc++, added to the headers that use them; `Exception.hh` includes `fmt/format.h` for fmt 12 |
| `fmt` | 12.2.0.bcr.1 | none | Raises avro-cpp's fmt to a release with the libc++ include fix |
| `gcc_toolchain` | 0.13.0 | `sanitizer_spec.patch` | Ships `libsanitizer.spec` so GCC `-fsanitize=` links in the sandbox |
| `gcc_toolchain` | 0.13.0 | `runtime_libs.patch` | Exposes the GCC runtime libraries (libstdc++, libgcc) to link actions |
| `gcc_toolchain` | 0.13.0 | `target_identity_config.patch` | `cc_toolchain_config` takes `target_libc` and `target_system_name` and reports them instead of `local` (#6575; upstream: f0rmiga/gcc-toolchain#257, commit 08f057a; drop when a gcc_toolchain release has it, #7704) |
| `gcc_toolchain` | 0.13.0 | `target_identity_defs.patch` | Repository rule passes `target_libc` and `<arch>-linux-gnu` into the generated toolchain (#6575; upstream: f0rmiga/gcc-toolchain#257, commit 08f057a; drop when a gcc_toolchain release has it, #7704) |
| `gcc_toolchain` | 0.13.0 | `target_identity_extension.patch` | Module extension forwards `target_libc` and the target system name to the repository rule (#6575; upstream: f0rmiga/gcc-toolchain#257, commit 08f057a; drop when a gcc_toolchain release has it, #7704) |
| `gcc_toolchain` | 0.13.0 | `native_include_layout.patch` | `cxx_builtin_includes` lists the C++ include dirs whenever the toolchain ships `include/c++`, not only on x86_64 |
| `llvm` | 0.8.26 | `libunwind_copy.patch` | `libunwind.a` is a real copy of the runtime archive, not a symlink that dangles when the archive is gone from a reused output base (#7781) |
| `llvm` | 0.8.26 | `clang_cl_crt_default_lib.patch` | clang-cl links name `msvcrt.lib` or `libcmt.lib` as a default library, so a `cc_binary` whose `main` sits in a dependency, like grpc's `protoc`, finds `mainCRTStartup` (#8223) |
| `llvm` | 0.8.26 | `unwatched_overlay_inputs.patch` | `llvm-project` watches its patch and overlay tree once instead of recording each file, so a warm build reaches the repo contents cache hit in a few steps, not 37 (#8986; upstream: hermeticbuild/hermetic-llvm#917 and #918, bazelbuild/bazel#32478, tracked in #9008) |
| `llvm` | 0.8.26 | `lse_atomic_sources.patch` | The aarch64 builtins' `lse_<pat>_<size>_<model>.S` copies of `lse.S` are written when `llvm-project` is fetched, not 125 symlink actions per configuration (#9080; upstream: hermeticbuild/hermetic-llvm#922, tracked in #9091; drop when an llvm release has it) |
| `llvm` | 0.8.26 | `msvc_source_directories.patch` | The MSVC header and library subsets that clang-cl searches are aliases to directories `@msvc_runtime` writes when fetched, not `copy_to_directory` actions (#8912; upstream: hermeticbuild/hermetic-llvm#919 and PR #920; drop when an llvm release has it) |
| `llvm` | 0.8.26 | `stage0_runtime_features.patch` | Stage0 runtime objects reset the same features as the bootstrap transition, so they share one configuration: `//cli/process/...` configures about 71k targets on Linux instead of 84k (#9202; upstream: hermeticbuild/hermetic-llvm#925; drop when an llvm release has it) |
| `llvm` | 0.8.26 | `aarch64_outline_atomics.patch` | The aarch64 builtins skip the `__aarch64_*` outline atomic helpers on macOS and Windows, where clang never calls them (#9125; upstream: hermeticbuild/hermetic-llvm#923 and #924; drop when an llvm release has it) |
| `windows_support` | 0.4.1 | `host_tools.patch` | Opt-in `host_tools` tag keeps the MSVC compiler tools and Windows SDK `bin` (hermeticbuild/windows_support#19) |
| `windows_support` | 0.4.1 | `rules_dx_hosts.patch` | `host_tools` is on by default under `rules_dx`, so `@dx_msvc` has `cl.exe`; child names match case-insensitively, since payload directory case differs from `Hostx64` |
| `windows_support` | 0.4.1 | `dx_msvc_layout.patch` | `@msvc_runtime` writes the MSVC header and library subsets that llvm's clang-cl toolchain searches and exposes them as `subdirectory` targets (#8912; upstream: hermeticbuild/windows_support#20 and PR #21; drop when a windows_support release has it) |
| `windows_support` | 0.4.1 | `source_directory_headers.patch` | The Windows SDK `ucrt`, `shared`, `um` and `winrt` include targets give compiles one source directory each instead of about 4,000 loose headers, so Bazel caches their Merkle trees (#9135; upstream: hermeticbuild/windows_support#22 and PR #23; drop when a windows_support release has it) |
| `windows_support` | 0.4.1 | `hostarm64_packages.patch` | `@msvc_runtime` selects the `Hostarm64` MSVC packages and their dependencies when `host_tools` is set |

The trade-off: a `single_version_override` pins the module for the whole
graph. If another dependency of yours needs a newer version of one of these
modules, Bazel still resolves the version above. Move the `rules_dx` pin
instead, or drop the `include()` and own the override yourself.

## elvis, rules_d, rules_dotnet, rules_elixir, rules_erlang, rules_fortran, rules_haskell, rules_ocaml, rules_powershell, rules_ruby

No action. `rules_dx` applies these patches in its own repository rules.

## ring (crate annotation)

`MODULE.bazel` patches ring 0.17.14's `build.rs` so it accepts clang-cl on
`aarch64-pc-windows-msvc` instead of looking for plain `clang` (#6337). A
second patch includes the fiat headers from the crate root, because cl.exe
opens `../../third_party/fiat/...` without collapsing the `..` parts and the
path passes 260 characters under the build script runfiles (#8290). Both are
applied to rules_dx's own `dx_crates` and are not exported to consumers.

## gomod-parser (crate annotation)

`MODULE.bazel` patches gomod-parser 0.5.3 so a require version followed
directly by `//` parses (#4008, baz-scm/gomod-parser#52). It is applied to
rules_dx's own `dx_crates` and needs nothing from consumers. Remove it once a
gomod-parser release after 0.6.0 carries the fix.
