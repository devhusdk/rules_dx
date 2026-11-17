# Patches

This page helps you add, change or drop a patch that rules_dx applies to another Bazel module. A Bazel module is a dependency declared with `bazel_dep` in `MODULE.bazel`.

## Add a patch to a module

1. Put the patch file in `patches/<module>/`, for example `patches/rules_rust/my_fix.patch`, and add it to `exports_files` in that directory's `BUILD.bazel`.
2. Add its label, such as `//patches/rules_rust:my_fix.patch`, to the `patches` list of the module's `single_version_override` in `MODULE.bazel`.
3. Add a row to the table below. Leave the Version cell as it is; the next step fills it in.
4. Refresh the page:

   ```sh
   bazel run //patches:update_readme
   ```

   It copies each Version from `MODULE.bazel` and fails when a patch that `MODULE.bazel` applies is missing from this page, or a row names a patch that `MODULE.bazel` does not apply.

Bazel applies a `single_version_override` only in the root module, the one you run Bazel in. So these patches change builds inside this repository, not a consumer's build. The Why cell of each row says which kind it is:

- It starts with `Root only.` when only rules_dx's own builds need the patch.
- It ends with `Consumer fix: #N` when consumers need the fix too. `#N` is the open issue that makes consumers work without the patch.

When the upstream fix is in a release rules_dx uses, delete the patch file, its label and its row, then run `bazel run //patches:update_readme` again.

## Patches in MODULE.bazel overrides

| Module | Version | Patches | Why |
| --- | --- | --- | --- |
| `rules_rust` | 0.75.0 | `clippy_metadata_inputs.patch` | Root only. clippy reads dependency `.rmeta` |
| `rules_rust` | 0.75.0 | `pipelined_rmeta_path.patch` | Root only. The full rustc action writes its own `.rmeta` so it does not hit the read-only one from the metadata action (bazelbuild/rules_rust#3383; drop when a rules_rust release has it) |
| `rules_rust` | 0.75.0 | `msvc_pdb_output.patch` | Root only. MSVC `bin` and `cdylib` crates always declare their `.pdb`, since rustc writes it whatever the strip level (rust-lang/rust#115120); without it an opt build still succeeds, with no `pdb_file` output group |
| `rules_rust` | 0.75.0 | `crate_universe_lazy_host_tools.patch` | Root only. `crate` hubs with a `lockfile` render from it without fetching `rust_host_tools` (cargo and rustc), unless a repin is requested or `CARGO_BAZEL_DIGEST_CHECK=1` is set, which CI preflight does |
| `rules_rust` | 0.75.0 | `cargo_toml_env_vars_flag.patch` | Root only. `--@rules_rust//cargo/settings:cargo_toml_env_vars` defaults to `False`: `cargo_toml_env_vars` writes an empty env file, so `cargo_toml_variable_extractor` and the 55 crates it needs are never fetched (drop when a rules_rust release has the flag) |
| `rules_rust` | 0.75.0 | `exec_scope_universal.patch` | Root only. `toolchain_linker_preference` and `use_default_shell_env` have `scope = "universal"`, so exec tools keep reading them under that flag, and `use_default_shell_env` defaults to `False` (drop when a rules_rust release has it) |
| `rules_rust` | 0.75.0 | `static_archive_link_arg.patch` | The trailing link flag of a native static library is its archive path, not `-l<name>`, so a `lib<name>.so` next to the archive is never linked when Rustc runs unsandboxed (bazelbuild/rules_rust#3993; drop when a rules_rust release has it). Consumer fix: #12010 |
| `rules_rust` | 0.75.0 | `cargo_build_script_no_coverage.patch` | Build scripts configure cc features without `coverage`, `gcc_coverage_map_format` and `llvm_coverage_map_format`, so apple_support's unconditional `GCOV_GCNO_FILE=%{gcov_gcno_file}` env entry does not fail analysis on macOS under coverage (drop when a rules_rust release has it). Consumer fix: #12155 |
| `rules_rust` | 0.75.0 | `crate_universe_module_labels.patch` | Root only. Crate hub lockfiles pin the root module's own labels as `@@<module>+//`, so the pinned lockfile resolves when rules_dx is a dependency; the root build reads them back as `@@//` |
| `rules_rust` | 0.75.0 | `windows_exec_foreign_target_cc_linker.patch` | Root only. A generated `rust_toolchain` with a Windows exec triple and a non-Windows target triple sets `linker_preference = "cc"`, so those targets link through the cc toolchain; one with a non-Windows exec triple, an MSVC target triple and `rust-lld` sets `linker_preference = "rust"`, so those targets link with `rust-lld` as `lld-link` |
| `rules_android` | 0.8.1 | `bundle_keystore_properties.patch` | App Bundles with key rotation: write the oldest-signer properties file instead of reading the removed `_bundle_keystore_properties` attribute. Consumer fix: #11417 |
| `rules_android` | 0.8.1 | `java8_dex_runfiles.patch` | `build_java8_legacy_dex.sh` sets `JAVA_RUNFILES` to its own runfiles tree, as `d8_desugar.sh` does, so the `d8`, `r8` and `tracereferences` launchers it resolves through the runfiles manifest find their jars when the genrule runs unsandboxed. Consumer fix: #11387 |
| `rules_scala` | 7.4.1 | `manifest_only_wrapper.patch` | The `scala_binary` launcher finds its java wrapper through `RUNFILES_MANIFEST_FILE` when the runfiles directory does not have it, and sets `TEST_SRCDIR` from `JAVA_RUNFILES` when only that is set. Consumer fix: #11417 |
| `rules_cuda` | 0.4.0 | `msvc_dlink_crt.patch` | `nvcc -dlink` on Windows gets the same `/MD` or `/MT` and `-ccbin` host compiler as the compile action, so rdc links do not mix LIBCMT with the dynamic CRT. Consumer fix: #11418 |
| `rules_cuda` | 0.4.0 | `toolchains_gate.patch` | Root only. The CUDA toolchains have `target_settings = ["@@//config:toolchains_on"]` |
| `grpc` | 1.85.1 | `pip_parse_no_310.patch` | Root only. grpc's pip hub skips python 3.10, which has no aarch64-pc-windows-msvc runtime, so windows_arm64 lock refresh evaluates |
| `grpc` | 1.85.1 | `upb_reflection_cc.patch` | Root only. Targets that depend on `@com_google_protobuf//upb/reflection` also depend on `upb/reflection:reflection_cc`, which has `def.hpp` and `message.hpp` from protobuf 37.0 on |
| `protoc-gen-validate` | 1.2.1.bcr.2 | `pip_parse_no_39_310.patch` | Root only. pgv's pip hub skips python 3.9 and 3.10, which have no aarch64-pc-windows-msvc runtime |
| `rules_dart` | 0.8.1 | `coverage.patch` | `dart_test` writes LCOV under `bazel coverage`: the test launcher passes `--coverage` to `package:test` and converts the report. Only a test built with coverage collects it, so a subject built without `--collect_code_coverage` reports none. Consumer fix: #11417 |
| `rules_dart` | 0.8.1 | `external_pkgsrcs.patch` | Sources of packages from an external repository are staged into `.pkgsrcs`, which `copy_to_directory` otherwise drops. Consumer fix: #11417 |
| `rules_dart` | 0.8.1 | `windows_long_dill.patch` | Root only. On Windows `test_runner.dart` resolves the symlinked kernel snapshot before starting the test |
| `rules_java` | 9.9.0 | `toolchains_gate.patch` | Root only. The default Java toolchain definitions have `target_settings = ["@@//config:toolchains_on", ...]` |
| `rules_flutter` | 0.0.8 | `lock_extension.patch` | Root only. The `flutter` extension is not reproducible, so a lockfile written on one host does not pin another |
| `rules_flutter` | 0.0.8 | `registrant_javacopts.patch` | The Android plugin registrant compiles without `-Xlint:all` warnings about source 8. Consumer fix: #11417 |
| `rules_flutter` | 0.0.8 | `required_checksums.patch` | A Flutter engine download without a sha256 fails the fetch, including the Dart SDK of the web SDK repo. Consumer fix: #11417 |
| `rules_flutter` | 0.0.8 | `flutter_3_47_9.patch` | Root only. `FLUTTER_VERSIONS` and `ARTIFACT_CHECKSUMS` list Flutter 3.47.9, which no rules_flutter release has yet |
| `rules_flutter` | 0.0.8 | `ios_target_compatible_with.patch` | `flutter_ios_app` applies its `target_compatible_with` to every target it generates, not only drops it. Consumer fix: #11417 |
| `rules_flutter` | 0.0.8 | `coverage_enabled.patch` | Root only. Only a `flutter_test` built with `--collect_code_coverage` writes LCOV; one built without it ignores `COVERAGE_OUTPUT_FILE` |
| `rules_swift` | 4.2.2 | `clang_cl_no_split_cold_code.patch` | The exec worker builds with LLVM clang-cl, which rejects the Apple-only `-fno-split-cold-code` (bazelbuild/rules_swift#2005). Consumer fix: #11817 |
| `rules_swift` | 4.2.2 | `windows_absolute_sdk.patch` | Root only. Windows Swift actions pass `-sdk` through the wrapper as an absolute path |
| `rules_swift` | 4.2.2 | `windows_swift_testing.patch` | Root only. The Windows toolchain puts the Swift Testing module and import library directories on the compile and link paths (bazelbuild/rules_swift#2018; drop when a rules_swift release has it) |
| `rules_swift` | 4.2.2 | `windows_unknown_vendor.patch` | Root only. Windows target triples use the `unknown` vendor |
| `rules_swift` | 4.2.2 | `xcode_select_absolute_path.patch` | The `system_sdk` extension runs `/usr/bin/xcode-select`, so Xcode discovery works with no host `PATH`. Consumer fix: #11150 |
| `rules_python` | 2.4.6 | `single_import_lib.patch` | `py_extension` on Windows links one CPython import library: the stable ABI one for limited API modules, else the versioned one. It passed both as one linker argument (bazel-contrib/rules_python#4426; drop when a rules_python release has it). Consumer fix: #11418 |
| `aspect_rules_js` | 3.6.0 | `manifest_only_runfiles.patch` | Root only. The `js_binary` bash launcher accepts a `RUNFILES_MANIFEST_FILE` at any path and builds its runfiles tree from the manifest when there is no runfiles directory beside it |
| `rules_python` | 2.4.6 | `install_name_tool.patch` | Root only. The macOS `python_repository` skips the LC_ID_DYLIB fixup, which needs `install_name_tool` on PATH, when `libpython` already names itself `@rpath/` |
| `rules_python` | 2.4.6 | `lock_extension.patch` | Root only. The `python` extension is OS and architecture dependent and not reproducible, so a lockfile written on one host does not pin another |
| `rules_py` | 2.0.0-alpha.11 | `py_cc_toolchain_platform.patch` | `py_cc_toolchain` picks the cc toolchain for the target platform. Consumer fix: #11398 |
| `rules_py` | 2.0.0-alpha.11 | `native_sdist_toolchain_flags.patch` | Native sdist builds get the cc toolchain flags (aspect-build/rules_py#1664). Consumer fix: #11398 |
| `rules_py` | 2.0.0-alpha.11 | `pep517_msvc.patch` | PEP 517 builds find the MSVC toolchain on Windows (aspect-build/rules_py#1662). Consumer fix: #11398 |
| `rules_py` | 2.0.0-alpha.11 | `venv_bin_files.patch` | The runtime sets `venv_bin_files`, so `java_binary` analyzes on Windows; there the venv's `python.exe` and DLLs are file symlinks and wheels join `sys.path` through `.pth` lines, so the launcher does not hit `CreateProcess failed` and wheel imports resolve (aspect-build/rules_py#1670). Consumer fix: #11398 |
| `rules_py` | 2.0.0-alpha.11 | `host_interpreter_windows_arm64.patch` | rules_py can see its `python_3_13_aarch64_pc_windows_msvc` interpreter, so sdist builds load on Windows ARM64 hosts (aspect-build/rules_py#1676). Consumer fix: #11398 |
| `rules_py` | 2.0.0-alpha.11 | `external_repo_pytest_paths.patch` | `py_pytest_main` finds `.pytest_paths` for targets in an external repository under `../<repo>/<package>`. Consumer fix: #11398 |
| `rules_py` | 2.0.0-alpha.11 | `host_libc_glibc.patch` | The uv host repo reports glibc 2.28 on Linux without running `ldd` and libsystem 14.0 on macOS without running `sw_vers`, so it fetches where neither is on PATH (aspect-build/rules_py#1686). Consumer fix: #12482 |
| `rules_py` | 2.0.0-alpha.11 | `win32_x86_32.patch` | The `win32` wheel tag matches `@platforms//cpu:x86_32`, so x86_64 Windows selects the `win_amd64` wheel (aspect-build/rules_py#1685). Consumer fix: #12277 |
| `hermetic_launcher` | 0.0.17 | `windows_arm64_toolchains.patch` | Registers template and finalizer toolchains for the prebuilt aarch64 Windows stubs (hermeticbuild/hermetic-launcher#94). Consumer fix: #11416 |
| `avro-cpp` | 1.12.0.bcr.1 | `iterator.patch` | Missing std includes (`<iterator>`, `<memory>`, `<vector>`) with libc++, added to the headers that use them; `Exception.hh` includes `fmt/format.h` for fmt 12. Consumer fix: #6592 |
| `fmt` | 12.2.0.bcr.1 | none | Raises avro-cpp's fmt to a release with the libc++ include fix. Consumer fix: #6592 |
| `gcc_toolchain` | 0.13.0 | `sanitizer_spec.patch` | Root only. Ships `libsanitizer.spec` so GCC `-fsanitize=` links in the sandbox. Root-only: `cc/private/toolchain/gcc_repo/` vendors the patched repository rule, so consumers use BCR 0.13.0 unpatched (upstream f0rmiga/gcc-toolchain#253) |
| `gcc_toolchain` | 0.13.0 | `runtime_libs.patch` | Root only. Exposes the GCC runtime libraries (libstdc++, libgcc) to link actions. Root-only: `cc/private/toolchain/gcc_repo/` vendors the patched repository rule, so consumers use BCR 0.13.0 unpatched (upstream f0rmiga/gcc-toolchain#253) |
| `gcc_toolchain` | 0.13.0 | `target_identity_config.patch` | Root only. `cc_toolchain_config` takes `target_libc` and `target_system_name` and reports them instead of `local` (upstream: f0rmiga/gcc-toolchain#257, commit 08f057a; drop when a gcc_toolchain release has it). Root-only: `cc/private/toolchain/gcc_repo/` vendors the patched repository rule, so consumers use BCR 0.13.0 unpatched (upstream f0rmiga/gcc-toolchain#253) |
| `gcc_toolchain` | 0.13.0 | `target_identity_defs.patch` | Root only. Repository rule passes `target_libc` and `<arch>-linux-gnu` into the generated toolchain (upstream: f0rmiga/gcc-toolchain#257, commit 08f057a; drop when a gcc_toolchain release has it). Root-only: `cc/private/toolchain/gcc_repo/` vendors the patched repository rule, so consumers use BCR 0.13.0 unpatched (upstream f0rmiga/gcc-toolchain#253) |
| `gcc_toolchain` | 0.13.0 | `target_identity_extension.patch` | Root only. Module extension forwards `target_libc` and the target system name to the repository rule (upstream: f0rmiga/gcc-toolchain#257, commit 08f057a; drop when a gcc_toolchain release has it). Root-only: `cc/private/toolchain/gcc_repo/` vendors the patched repository rule, so consumers use BCR 0.13.0 unpatched (upstream f0rmiga/gcc-toolchain#253) |
| `gcc_toolchain` | 0.13.0 | `native_include_layout.patch` | Root only. `cxx_builtin_includes` lists the C++ include dirs whenever the toolchain ships `include/c++`, not only on x86_64. Root-only: `cc/private/toolchain/gcc_repo/` vendors the patched repository rule, so consumers use BCR 0.13.0 unpatched (upstream f0rmiga/gcc-toolchain#253) |
| `llvm` | 0.8.27 | `libunwind_copy.patch` | `libunwind.a` is a real copy of the runtime archive, not a symlink that dangles when the archive is gone from a reused output base. Consumer fix: #11416 |
| `llvm` | 0.8.27 | `clang_cl_profile_no_mingw.patch` | `compiler-rt:profile` takes no mingw headers on the windows msvc ABI, so clang-cl's `stdint.h` `#include_next` finds the MSVC CRT. Consumer fix: #11416 |
| `llvm` | 0.8.27 | `unwatched_overlay_inputs.patch` | `llvm-project` watches its patch and overlay tree once instead of recording each file, so a warm build reaches the repo contents cache hit in a few steps, not 37 (upstream: hermeticbuild/hermetic-llvm#917, bazelbuild/bazel#32478). Consumer fix: #11416 |
| `llvm` | 0.8.27 | `lse_atomic_sources.patch` | The aarch64 builtins' `lse_<pat>_<size>_<model>.S` copies of `lse.S` are written when `llvm-project` is fetched, not 125 symlink actions per configuration (upstream: hermeticbuild/hermetic-llvm#922; drop when an llvm release has it). Consumer fix: #11416 |
| `llvm` | 0.8.27 | `msvc_source_directories.patch` | The MSVC header and library subsets that clang-cl searches are aliases to directories `@msvc_runtime` writes when fetched, not `copy_to_directory` actions (upstream: hermeticbuild/hermetic-llvm#919; drop when an llvm release has it). Consumer fix: #11416 |
| `llvm` | 0.8.27 | `stage0_runtime_features.patch` | Stage0 runtime objects reset the same features as the bootstrap transition, so they share one configuration: `//cli/process/...` configures about 71k targets on Linux instead of 84k (upstream: hermeticbuild/hermetic-llvm#925; drop when an llvm release has it). Consumer fix: #11416 |
| `llvm` | 0.8.27 | `macos_sdk_resolve_symlinks.patch` | The macOS SDK repo copies file symlinks, drops dangling ones, and flattens each `.framework` to its `Versions/Current` contents when fetched, so the sysroot has no symlinks and each header has one path; each framework keeps its `.tbd` at `Versions/<version>/` so ld64.lld resolves re-exports by install name. Consumer fix: #11416 |
| `llvm` | 0.8.27 | `msvc_fuzzer.patch` | The MSVC ABI accepts the `fuzzer` feature and stages `clang_rt.fuzzer*`; compiler-rt builds libFuzzer against libc++ headers without mingw headers; `asan` stays unsupported (drop when an llvm release has it). Consumer fix: #11416 |
| `windows_support` | 0.4.1 | `host_tools.patch` | Opt-in `host_tools` tag keeps the MSVC compiler tools and Windows SDK `bin` (hermeticbuild/windows_support#19). Consumer fix: #11416 |
| `windows_support` | 0.4.1 | `rules_dx_hosts.patch` | `host_tools` is on by default under `rules_dx`, so `@dx_msvc` has `cl.exe`; child names match case-insensitively, since payload directory case differs from `Hostx64`. Consumer fix: #11416 |
| `windows_support` | 0.4.1 | `dx_msvc_layout.patch` | `@msvc_runtime` writes the MSVC header and library subsets that llvm's clang-cl toolchain searches and exposes them as `subdirectory` targets (upstream: hermeticbuild/windows_support#20 and PR #21; drop when a windows_support release has it). Consumer fix: #11416 |
| `windows_support` | 0.4.1 | `source_directory_headers.patch` | The Windows SDK `ucrt`, `shared`, `um` and `winrt` include targets give compiles one source directory each instead of about 4,000 loose headers, so Bazel caches their Merkle trees (upstream: hermeticbuild/windows_support#22 and PR #23; drop when a windows_support release has it). Consumer fix: #11416 |
| `windows_support` | 0.4.1 | `hostarm64_packages.patch` | `@msvc_runtime` selects the `Hostarm64` MSVC packages and their dependencies when `host_tools` is set. Consumer fix: #11416 |
| `windows_support` | 0.4.1 | `reproducible_extensions.patch` | Root only. `msvc_runtime` and `windows_sdk` are reproducible when their payloads are pinned by integrity, so `MODULE.bazel.lock` does not record them |
| `windows_support` | 0.4.1 | `dx_cc_extension.patch` | Root only. `msvc_runtime.bzl` and `windows_sdk.bzl` export the repository functions and configure attributes that the `cc` extension calls |
| `windows_support` | 0.4.1 | `license_ids.patch` | Root only. The `msvc_runtime` repository fails unless the accepted license ids file, from the `license` tag or the `cc` extension, lists `visual-studio-build-tools`, and reads no environment variable |
| `windows_support` | 0.4.1 | `pins.patch` | Root only. `msvc_runtime` reads the Visual Studio installer manifest URL and integrity from the `pins` file when it is set |
| `apple_support` | 2.12.0 | `gcov_gcno_file.patch` | `GCOV_GCNO_FILE` is set only when `gcov_gcno_file` is available, so `cc_common.get_environment_variables` without it (rules_go `cgo_context_data`, rules_rust build scripts) does not fail analysis on macOS under coverage (drop when an apple_support release has it). Consumer fix: #12155 |
| `apple_support` | 2.12.0 | `toolchains_gate.patch` | Root only. The Apple C++ toolchains have `target_settings = ["@@//config:toolchains_on"]` |
| `apple_support` | 2.12.0 | `xcode_absolute_tools.patch` | Xcode and C++ toolchain discovery run `/usr/bin/env` and `/usr/bin/xcrun`, so they work with no host `PATH`. Consumer fix: #11150 |
| `elfutils` | 0.195 | `musl.patch` | Root only. libdwfl compiles without argp, fts and GNU `strerror_r` on musl, so `//tools/coverage/kcov:kcov_static` links fully static |
| `rules_go` | 0.65.0 | `lazy_nogo.patch` | Toolchain registration does not load the Go rules, so builds without Go targets never fetch `@io_bazel_rules_nogo`. Consumer fix: #11418 |
| `rules_go` | 0.65.0 | `lock_extension.patch` | Root only. The `go_sdk` extension is not reproducible, so the lockfile keeps its result per platform |
| `rules_oci` | 2.3.0 | `descriptor_image.patch` | On a Windows exec platform each layer descriptor action of `oci_image` gets its own `.bat` wrapper and lists `descriptor.sh` as a tool. Consumer fix: #11758 |
| `rules_oci` | 2.3.0 | `descriptor_util.patch` | The Windows `.bat` wrapper takes a name and calls a script outside its own directory by its execroot path. Consumer fix: #11758 |
| `rules_oci` | 2.3.0 | `image_index.patch` | `oci_image_index` wraps its shell launcher in a `.bat` on a Windows exec platform. Consumer fix: #11758 |
| `rules_oci` | 2.3.0 | `image_index_blob.patch` | `oci_image_index` skips blobs already copied from a sibling image. Consumer fix: #11758 |
| `rules_oci` | 2.3.0 | `image_layout.patch` | `oci_image` writes the base layout's `oci-layout` as a fresh file so regctl can rewrite it on Windows. Consumer fix: #11758 |

## Patches in rules_dx repository rules: elvis, rules_d, rules_dart, rules_dotnet, rules_elixir, rules_erlang, rules_flutter, rules_fortran, rules_haskell, rules_julia, rules_ocaml, rules_ruby, windows_support

rules_dx applies these patches in its own repository rules, so consumers get them too and you need to do nothing: `lazy_format.patch` (elvis); `pic_link.patch`, `disabled_features.patch` (rules_d); `hermetic_tar.patch` (rules_dart); `module_map_key.patch` (rules_fortran); `engine_labels.patch`, `hermetic_pub.patch` (rules_flutter); `reproducible.patch`, `fsharp_signature.patch`, `nuget_group_prefix.patch`, `sdk_versions.patch`, `canonical_labels.patch`, `toolchains_gate.patch`, `launcher_args.patch`, `target_runtime.patch` (rules_dotnet); `coverage_wrapper_no_report.patch`, `hermetic_haskell.patch`, `cabal_rules.patch`, `cabal_mingw_transition.patch`, `cc_wrapper_relpath_mount.patch`, `ghc_wrapper_runfiles_env.patch`, `cc_toolchain_config_info.patch`, `cabal_llvm_ar.patch`, `cc_wrapper_darwin_ffi.patch`, `cabal_binary_dep_data.patch`, `cabal_static_exe.patch` (rules_haskell); `elixir_app_deps.patch`, `elixir_app_name.patch`, `relocatable_tree.patch`, `warnings_as_errors.patch`, `windows_ex_unit_test.patch` (rules_elixir); `01_xrl_yrl.patch`, `02_tcw_tags.patch`, `03_auto_sname.patch`, `04_otp_tree.patch`, `05_windows_eunit_cover.patch`, `06_coverage_enabled.patch`, `07_windows_ct_cover.patch` (rules_erlang); `config_base_toml.patch`, `package_annotation_version.patch`, `package_name.patch`, `portable_cpu_target.patch`, `template_action_input.patch` (rules_julia); `hermetic_ocaml.patch`, `native_cc_env.patch`, `platform_alias.patch`, `static_cc_deps.patch` (rules_ocaml); `reproducible.patch`, `coverage.patch`, `rubyinstaller_archive.patch`, `toolchains_gate.patch`, `toolchains_hub.patch`, `windows_binary_rlocation.patch` (rules_ruby).

## kcov

`MODULE.bazel` patches the kcov v43 archive for `//tools/coverage/kcov`: `aslr.patch` keeps running when ASLR cannot be disabled, `elf_checksum_lock.patch` loads ELF sections before the checksum so elfutils built with locks does not deadlock, and `no_curl.patch` drops the libcurl dependency.

## libfuzzer_dotnet

`macos_section.patch` puts the extra counters of `libfuzzer-dotnet.cc` in `__DATA,__libfuzz_extra` on macOS. `snprintf.patch` replaces the deprecated `sprintf` call with `snprintf`. Both apply to the `dx_libfuzzer_dotnet` archive.

## capnproto

`dynamic_cstddef.patch` applies to the `dx_capnproto` source archive.

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
