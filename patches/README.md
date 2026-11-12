# Patches

The `single_version_override`s in rules_dx's own `MODULE.bazel`, with the patches under `patches/`. Bazel honours them only in the root module, so they apply to builds inside this repository. Each Why cell either starts with `Root only.`, for a patch that only rules_dx's own builds need, or ends with `Consumer fix: #N`, the open issue that makes consumers work without the patch.

The Version column is generated from `MODULE.bazel`, and every patch it applies must appear in the table. Refresh it with `bazel run //patches:update_readme`.

| Module | Version | Patches | Why |
| --- | --- | --- | --- |
| `rules_rust` | 0.75.0 | `clippy_metadata_inputs.patch` | Root only. clippy reads dependency `.rmeta` |
| `rules_rust` | 0.75.0 | `pipelined_rmeta_path.patch` | Root only. The full rustc action writes its own `.rmeta` so it does not hit the read-only one from the metadata action (bazelbuild/rules_rust#3383; drop when a rules_rust release has it) |
| `rules_rust` | 0.75.0 | `crate_universe_root_lookup.patch` | Root only. `crate_universe` looks up the root `MODULE.bazel` only in the root module, so a consumer without a root `BUILD.bazel` can use `dx_crates` |
| `rules_rust` | 0.75.0 | `msvc_pdb_output.patch` | MSVC `bin` and `cdylib` crates always declare their `.pdb`, since rustc writes it whatever the strip level (rust-lang/rust#115120). Consumer fix: #11414 |
| `rules_rust` | 0.75.0 | `crate_universe_lazy_host_tools.patch` | `crate` hubs with a `lockfile` render from it without fetching `rust_host_tools` (cargo and rustc), unless a repin is requested or `CARGO_BAZEL_DIGEST_CHECK=1` is set, which CI preflight does. Consumer fix: #11414 |
| `rules_rust` | 0.75.0 | `cargo_toml_env_vars_flag.patch` | `--@rules_rust//cargo/settings:cargo_toml_env_vars=False` makes `cargo_toml_env_vars` write an empty env file, so `cargo_toml_variable_extractor` and the 55 crates it needs are never fetched (drop when a rules_rust release has the flag). Consumer fix: #11414 |
| `rules_rust` | 0.75.0 | `exec_scope_universal.patch` | Root only. `toolchain_linker_preference` and `use_default_shell_env` have `scope = "universal"`, so exec tools keep reading them under that flag (drop when a rules_rust release has it) |
| `rules_rust` | 0.75.0 | `static_archive_link_arg.patch` | The trailing link flag of a native static library is its archive path, not `-l<name>`, so a `lib<name>.so` next to the archive is never linked when Rustc runs unsandboxed (bazelbuild/rules_rust#3993; drop when a rules_rust release has it). Consumer fix: #12010 |
| `rules_rust` | 0.75.0 | `cargo_build_script_no_coverage.patch` | Build scripts configure cc features without `coverage`, `gcc_coverage_map_format` and `llvm_coverage_map_format`, so apple_support's unconditional `GCOV_GCNO_FILE=%{gcov_gcno_file}` env entry does not fail analysis on macOS under coverage (#12078; drop when a rules_rust release has it). Consumer fix: #12155 |
| `rules_android` | 0.8.1 | `bundle_keystore_properties.patch` | App Bundles with key rotation: write the oldest-signer properties file instead of reading the removed `_bundle_keystore_properties` attribute. Consumer fix: #11417 |
| `rules_kotlin` | 2.4.21 | `windows_coverage_test_suite.patch` | Windows `bazel coverage` of `kt_jvm_test`: the launcher no longer adds a second `-Dbazel.test_suite=<main class>` after the real one, which made `BazelTestRunner` run itself as the suite (bazel-contrib/rules_kotlin#1828; drop when a rules_kotlin release has it). `JACOCO_MAIN_CLASS` itself comes from `dx_jvm_test_coverage` in `//java/private:coverage.bzl`, since the Windows launcher ignores `jacoco_main_class` (bazelbuild/bazel#32479). Consumer fix: #11417 |
| `rules_cuda` | 0.4.0 | `msvc_dlink_crt.patch` | `nvcc -dlink` on Windows gets the same `/MD` or `/MT` and `-ccbin` host compiler as the compile action, so rdc links do not mix LIBCMT with the dynamic CRT. Consumer fix: #11418 |
| `protobuf` | 36.2 | `no_crate_universe.patch` | Drops protobuf's own `crate_universe` use so it does not clash with `dx_crates`. Consumer fix: #11418 |
| `grpc` | 1.85.1 | `pip_parse_no_310.patch` | Root only. grpc's pip hub skips python 3.10, which has no aarch64-pc-windows-msvc runtime, so windows_arm64 lock refresh evaluates (#12023) |
| `protoc-gen-validate` | 1.2.1.bcr.2 | `pip_parse_no_39_310.patch` | Root only. pgv's pip hub skips python 3.9 and 3.10, which have no aarch64-pc-windows-msvc runtime (#12023) |
| `rules_julia` | 0.8.0 | `package_name.patch`, `portable_cpu_target.patch`, `template_action_input.patch` | Julia `package_name` attribute for the wrapper targets (periareon/rules_julia#42); package precompiles target portable CPUs so a cache built on one host loads on another; the wrapper script is rendered by an action that reads the generated template as an input, so `julia_binary` builds without `--remote_download_outputs=all`. Consumer fix: #11418 |
| `rules_julia` | 0.8.0 | `package_annotation_version.patch` | Package annotations are looked up by `name@version` before `name`. Consumer fix: #11418 |
| `rules_dart` | 0.8.1 | `coverage.patch` | `dart_test` writes LCOV under `bazel coverage`: the test launcher passes `--coverage` to `package:test` and converts the report. Consumer fix: #11417 |
| `rules_dart` | 0.8.1 | `external_pkgsrcs.patch` | Sources of packages from an external repository are staged into `.pkgsrcs`, which `copy_to_directory` otherwise drops. Consumer fix: #11417 |
| `rules_flutter` | 0.0.8 | `lock_extension.patch` | Root only. The `flutter` extension is not reproducible, so a lockfile written on one host does not pin another |
| `rules_flutter` | 0.0.8 | `registrant_javacopts.patch` | The Android plugin registrant compiles without `-Xlint:all` warnings about source 8. Consumer fix: #11417 |
| `rules_flutter` | 0.0.8 | `required_checksums.patch` | A Flutter engine download without a sha256 fails the fetch, including the Dart SDK of the web SDK repo. Consumer fix: #11417 |
| `rules_flutter` | 0.0.8 | `flutter_3_47_9.patch` | Root only. `FLUTTER_VERSIONS` and `ARTIFACT_CHECKSUMS` list Flutter 3.47.9, which no rules_flutter release has yet |
| `rules_flutter` | 0.0.8 | `ios_target_compatible_with.patch` | `flutter_ios_app` applies its `target_compatible_with` to every target it generates, not only drops it. Consumer fix: #11417 |
| `rules_swift` | 4.2.2 | `clang_cl_no_split_cold_code.patch` | The exec worker builds with LLVM clang-cl, which rejects the Apple-only `-fno-split-cold-code` (bazelbuild/rules_swift#2005). Consumer fix: #11817 |
| `rules_swift` | 4.2.2 | `windows_absolute_sdk.patch` | Root only. Windows Swift actions pass `-sdk` through the wrapper as an absolute path |
| `rules_swift` | 4.2.2 | `windows_unknown_vendor.patch` | Root only. Windows target triples use the `unknown` vendor |
| `rules_python` | 2.4.6 | `single_import_lib.patch` | `py_extension` on Windows links one CPython import library: the stable ABI one for limited API modules, else the versioned one. It passed both as one linker argument (bazel-contrib/rules_python#4426; drop when a rules_python release has it). Consumer fix: #11418 |
| `rules_python` | 2.4.6 | `lock_extension.patch` | Root only. The `python` extension is OS and architecture dependent and not reproducible, so a lockfile written on one host does not pin another |
| `rules_py` | 2.0.0-alpha.11 | `py_cc_toolchain_platform.patch` | `py_cc_toolchain` picks the cc toolchain for the target platform. Consumer fix: #11398 |
| `rules_py` | 2.0.0-alpha.11 | `native_sdist_toolchain_flags.patch` | Native sdist builds get the cc toolchain flags (aspect-build/rules_py#1664). Consumer fix: #11398 |
| `rules_py` | 2.0.0-alpha.11 | `pep517_msvc.patch` | PEP 517 builds find the MSVC toolchain on Windows (aspect-build/rules_py#1662). Consumer fix: #11398 |
| `rules_py` | 2.0.0-alpha.11 | `venv_bin_files.patch` | The runtime sets `venv_bin_files`, so `java_binary` analyzes on Windows; there the venv's `python.exe` and DLLs are file symlinks, so the launcher does not hit `CreateProcess failed` (aspect-build/rules_py#1670). Consumer fix: #11398 |
| `rules_py` | 2.0.0-alpha.11 | `host_interpreter_windows_arm64.patch` | rules_py can see its `python_3_13_aarch64_pc_windows_msvc` interpreter, so sdist builds load on Windows ARM64 hosts (aspect-build/rules_py#1676). Consumer fix: #11398 |
| `rules_py` | 2.0.0-alpha.11 | `external_repo_pytest_paths.patch` | `py_pytest_main` finds `.pytest_paths` for targets in an external repository under `../<repo>/<package>`. Consumer fix: #11398 |
| `hermetic_launcher` | 0.0.17 | `windows_arm64_toolchains.patch` | Registers template and finalizer toolchains for the prebuilt aarch64 Windows stubs (hermeticbuild/hermetic-launcher#94). Consumer fix: #11416 |
| `avro-cpp` | 1.12.0.bcr.1 | `iterator.patch` | Missing std includes (`<iterator>`, `<memory>`, `<vector>`) with libc++, added to the headers that use them; `Exception.hh` includes `fmt/format.h` for fmt 12. Consumer fix: #6592 |
| `fmt` | 12.2.0.bcr.1 | none | Raises avro-cpp's fmt to a release with the libc++ include fix. Consumer fix: #6592 |
| `lua` | 5.5.0 | `overlay.patch` | Builds Lua 5.5.0 from the lua.org tarball with a BUILD overlay: `lua` library, `lua` and `luac` binaries, per-platform `LUA_USE_*` defines. Consumer fix: #11418 |
| `gcc_toolchain` | 0.13.0 | `sanitizer_spec.patch` | Root only. Ships `libsanitizer.spec` so GCC `-fsanitize=` links in the sandbox. Root-only: `cc/private/toolchains/gcc_repo/` vendors the patched repository rule, so consumers use BCR 0.13.0 unpatched (upstream f0rmiga/gcc-toolchain#253) |
| `gcc_toolchain` | 0.13.0 | `runtime_libs.patch` | Root only. Exposes the GCC runtime libraries (libstdc++, libgcc) to link actions. Root-only: `cc/private/toolchains/gcc_repo/` vendors the patched repository rule, so consumers use BCR 0.13.0 unpatched (upstream f0rmiga/gcc-toolchain#253) |
| `gcc_toolchain` | 0.13.0 | `target_identity_config.patch` | Root only. `cc_toolchain_config` takes `target_libc` and `target_system_name` and reports them instead of `local` (upstream: f0rmiga/gcc-toolchain#257, commit 08f057a; drop when a gcc_toolchain release has it). Root-only: `cc/private/toolchains/gcc_repo/` vendors the patched repository rule, so consumers use BCR 0.13.0 unpatched (upstream f0rmiga/gcc-toolchain#253) |
| `gcc_toolchain` | 0.13.0 | `target_identity_defs.patch` | Root only. Repository rule passes `target_libc` and `<arch>-linux-gnu` into the generated toolchain (upstream: f0rmiga/gcc-toolchain#257, commit 08f057a; drop when a gcc_toolchain release has it). Root-only: `cc/private/toolchains/gcc_repo/` vendors the patched repository rule, so consumers use BCR 0.13.0 unpatched (upstream f0rmiga/gcc-toolchain#253) |
| `gcc_toolchain` | 0.13.0 | `target_identity_extension.patch` | Root only. Module extension forwards `target_libc` and the target system name to the repository rule (upstream: f0rmiga/gcc-toolchain#257, commit 08f057a; drop when a gcc_toolchain release has it). Root-only: `cc/private/toolchains/gcc_repo/` vendors the patched repository rule, so consumers use BCR 0.13.0 unpatched (upstream f0rmiga/gcc-toolchain#253) |
| `gcc_toolchain` | 0.13.0 | `native_include_layout.patch` | Root only. `cxx_builtin_includes` lists the C++ include dirs whenever the toolchain ships `include/c++`, not only on x86_64. Root-only: `cc/private/toolchains/gcc_repo/` vendors the patched repository rule, so consumers use BCR 0.13.0 unpatched (upstream f0rmiga/gcc-toolchain#253) |
| `llvm` | 0.8.26 | `libunwind_copy.patch` | `libunwind.a` is a real copy of the runtime archive, not a symlink that dangles when the archive is gone from a reused output base. Consumer fix: #11416 |
| `llvm` | 0.8.26 | `clang_cl_crt_default_lib.patch` | clang-cl links name `msvcrt.lib` or `libcmt.lib` as a default library, so a `cc_binary` whose `main` sits in a dependency, like grpc's `protoc`, finds `mainCRTStartup`. Consumer fix: #11416 |
| `llvm` | 0.8.26 | `clang_cl_profile_no_mingw.patch` | `compiler-rt:profile` takes no mingw headers on the windows msvc ABI, so clang-cl's `stdint.h` `#include_next` finds the MSVC CRT. Consumer fix: #11416 |
| `llvm` | 0.8.26 | `unwatched_overlay_inputs.patch` | `llvm-project` watches its patch and overlay tree once instead of recording each file, so a warm build reaches the repo contents cache hit in a few steps, not 37 (upstream: hermeticbuild/hermetic-llvm#917, bazelbuild/bazel#32478). Consumer fix: #11416 |
| `llvm` | 0.8.26 | `lse_atomic_sources.patch` | The aarch64 builtins' `lse_<pat>_<size>_<model>.S` copies of `lse.S` are written when `llvm-project` is fetched, not 125 symlink actions per configuration (upstream: hermeticbuild/hermetic-llvm#922; drop when an llvm release has it). Consumer fix: #11416 |
| `llvm` | 0.8.26 | `msvc_source_directories.patch` | The MSVC header and library subsets that clang-cl searches are aliases to directories `@msvc_runtime` writes when fetched, not `copy_to_directory` actions (upstream: hermeticbuild/hermetic-llvm#919; drop when an llvm release has it). Consumer fix: #11416 |
| `llvm` | 0.8.26 | `stage0_runtime_features.patch` | Stage0 runtime objects reset the same features as the bootstrap transition, so they share one configuration: `//cli/process/...` configures about 71k targets on Linux instead of 84k (upstream: hermeticbuild/hermetic-llvm#925; drop when an llvm release has it). Consumer fix: #11416 |
| `llvm` | 0.8.26 | `aarch64_outline_atomics.patch` | The aarch64 builtins skip the `__aarch64_*` outline atomic helpers on macOS and Windows, where clang never calls them (upstream: hermeticbuild/hermetic-llvm#923; drop when an llvm release has it). Consumer fix: #11416 |
| `llvm` | 0.8.26 | `macos_sdk_resolve_symlinks.patch` | The macOS SDK repo copies file symlinks, drops dangling ones, and flattens each `.framework` to its `Versions/Current` contents when fetched, so the sysroot has no symlinks and each header has one path. Consumer fix: #11416 |
| `llvm` | 0.8.26 | `msvc_link_lib_env.patch` | MSVC link actions set `LIB` to the hermetic Windows SDK, MSVC CRT and libc++ lib dirs instead of an empty marker, so rules_rust passes each as `-LIBPATH:` and rust-lld finds `kernel32.lib` without a system Visual Studio. Consumer fix: #11967 |
| `windows_support` | 0.4.1 | `host_tools.patch` | Opt-in `host_tools` tag keeps the MSVC compiler tools and Windows SDK `bin` (hermeticbuild/windows_support#19). Consumer fix: #11416 |
| `windows_support` | 0.4.1 | `rules_dx_hosts.patch` | `host_tools` is on by default under `rules_dx`, so `@dx_msvc` has `cl.exe`; child names match case-insensitively, since payload directory case differs from `Hostx64`. Consumer fix: #11416 |
| `windows_support` | 0.4.1 | `dx_msvc_layout.patch` | `@msvc_runtime` writes the MSVC header and library subsets that llvm's clang-cl toolchain searches and exposes them as `subdirectory` targets (upstream: hermeticbuild/windows_support#20 and PR #21; drop when a windows_support release has it). Consumer fix: #11416 |
| `windows_support` | 0.4.1 | `source_directory_headers.patch` | The Windows SDK `ucrt`, `shared`, `um` and `winrt` include targets give compiles one source directory each instead of about 4,000 loose headers, so Bazel caches their Merkle trees (upstream: hermeticbuild/windows_support#22 and PR #23; drop when a windows_support release has it). Consumer fix: #11416 |
| `windows_support` | 0.4.1 | `hostarm64_packages.patch` | `@msvc_runtime` selects the `Hostarm64` MSVC packages and their dependencies when `host_tools` is set. Consumer fix: #11416 |
| `rules_go` | 0.65.0 | `lazy_nogo.patch` | Toolchain registration does not load the Go rules, so builds without Go targets never fetch `@io_bazel_rules_nogo`. Consumer fix: #11418 |
| `rules_go` | 0.65.0 | `lock_extension.patch` | Root only. The `go_sdk` extension is not reproducible, so the lockfile keeps its result per platform |
| `rules_flutter` | 0.0.8 | `lock_extension.patch` | Root only. The `flutter` extension is not reproducible, so the lockfile keeps its result |

## elvis, rules_d, rules_dart, rules_dotnet, rules_elixir, rules_erlang, rules_flutter, rules_fortran, rules_haskell, rules_ocaml, rules_ruby

No action. `rules_dx` applies these patches in its own repository rules: `hermetic_tar.patch` (rules_dart); `engine_labels.patch`, `hermetic_pub.patch` (rules_flutter); `reproducible.patch`, `fsharp_signature.patch`, `nuget_group_prefix.patch`, `sdk_versions.patch`, `canonical_labels.patch` (rules_dotnet); `reproducible.patch`, `coverage.patch`, `rubyinstaller_archive.patch` (rules_ruby).

## kcov

No action. `MODULE.bazel` patches the kcov v43 archive for `//tools/coverage/kcov`: `aslr.patch` keeps running when ASLR cannot be disabled, `elf_checksum_lock.patch` loads ELF sections before the checksum so elfutils built with locks does not deadlock, and `no_curl.patch` drops the libcurl dependency.

## libfuzzer_dotnet

No action. `macos_section.patch` puts the extra counters of `libfuzzer-dotnet.cc` in `__DATA,__libfuzz_extra` on macOS. It applies to the `dx_libfuzzer_dotnet` archive.

## capnproto

No action. `dynamic_cstddef.patch` applies to the `dx_capnproto` source archive.

## ring (crate annotation)

`MODULE.bazel` patches ring 0.17.14's `build.rs` so it accepts clang-cl on
`aarch64-pc-windows-msvc` instead of looking for plain `clang`. A
second patch includes the fiat headers from the crate root, because cl.exe
opens `../../third_party/fiat/...` without collapsing the `..` parts and the
path passes 260 characters under the build script runfiles. Both are
applied to rules_dx's own `dx_crates` and are not exported to consumers: `clang_cl_aarch64_windows.patch`, `msvc_fiat_include_path.patch`.

## gomod-parser (crate annotation)

`MODULE.bazel` patches gomod-parser 0.5.3 so a require version followed
directly by `//` parses (baz-scm/gomod-parser#52) with `glued_comment.patch`. It is applied to
rules_dx's own `dx_crates` and needs nothing from consumers. Remove it once a
gomod-parser release after 0.6.0 carries the fix.

## gotreesitter (go_repository)

`MODULE.bazel` patches the `com_github_odvcencio_gotreesitter` `go_repository` with `embed.patch`. It is applied to rules_dx's own build and needs nothing from consumers.
