# Patches

This page helps you add, change or drop a patch that rules_dx applies to another Bazel module. A Bazel module is a dependency declared with `bazel_dep` in `MODULE.bazel`.

## Add a patch to a module

1. Put the patch file in `patches/<module>/`, for example `patches/rules_rust/my_fix.patch`, and add it to `exports_files` in that directory's `BUILD.bazel`.
   Start the file with an `Upstream:` line above the diff that names the upstream pull request, or the issue when there is no pull request yet:

   ```text
   Upstream: bazelbuild/rules_rust#4390
   ```

   Name several with commas. When the fix landed upstream without a pull request, name its commit with the full sha: `Upstream: aran/rules_flutter@b12fd3fbfb130bb1b90ba03c790f3a8ce101363f`. When the fix will never go upstream, say why: `Upstream: none (rules_dx only: gates the toolchains behind //config:toolchains_on)`. Every Monday `.github/workflows/patch-upstream.yml` checks these lines and opens an issue when an upstream fix is in a release, so you know which patch to drop. A `bazelbuild/bazel-central-registry#N` ref counts as released once the pull request merges, so bump the module to the `.bcr.N` version it adds.
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

Some upstream fixes are carried as rules_dx code instead of a patch, such as extra toolchain args. Mark such a carry with a `# Upstream: owner/repo#N` comment line in its `BUILD.bazel` or `.bzl` file, above the code to delete. The weekly check reads these lines like patch headers. When the fix is in a release, bump the module and delete the carry and its marker.

## Patches in MODULE.bazel overrides

| Module | Version | Patches | Why |
| --- | --- | --- | --- |
| `rules_rust` | 0.75.0 | `clippy_metadata_inputs.patch` | Root only. clippy reads dependency `.rmeta` |
| `rules_rust` | 0.75.0 | `pipelined_rmeta_path.patch` | Root only. The full rustc action writes its own `.rmeta` so it does not hit the read-only one from the metadata action (bazelbuild/rules_rust#3383; drop when a rules_rust release has it) |
| `rules_rust` | 0.75.0 | `msvc_pdb_output.patch` | Root only. MSVC `bin` and `cdylib` crates always declare their `.pdb`, since rustc writes it whatever the strip level (rust-lang/rust#115120); without it an opt build still succeeds, with no `pdb_file` output group |
| `rules_rust` | 0.75.0 | `crate_universe_lazy_host_tools.patch` | Root only. `crate` hubs with a `lockfile` render from it without fetching `rust_host_tools` (cargo and rustc), unless a repin is requested or `CARGO_BAZEL_DIGEST_CHECK=1` is set, which CI preflight does |
| `rules_rust` | 0.75.0 | `cargo_toml_env_vars_flag.patch` | Root only. `--@rules_rust//cargo/settings:cargo_toml_env_vars` defaults to `False`: `cargo_toml_env_vars` writes an empty env file, so `cargo_toml_variable_extractor` and the 55 crates it needs are never fetched (drop when a rules_rust release has the flag) |
| `rules_rust` | 0.75.0 | `exec_scope_universal.patch` | Root only. `toolchain_linker_preference` and `use_default_shell_env` have `scope = "universal"`, so exec tools keep reading them under that flag, and `use_default_shell_env` defaults to `False` (drop when a rules_rust release has it) |
| `rules_rust` | 0.75.0 | `static_archive_link_arg.patch` | Root only. The trailing link flag of a native static library is its archive path, not `-l<name>`, so a `lib<name>.so` next to the archive is never linked when Rustc runs unsandboxed, as it does in the nested consumer tests under `--spawn_strategy=local` (bazelbuild/rules_rust#3993; drop when a rules_rust release has it). Consumers whose Rustc runs sandboxed, the default, never see the `.so`; the Native section of `rust/README.md` covers unsandboxed builds |
| `rules_rust` | 0.75.0 | `cargo_build_script_apple_deployment_target.patch` | Build scripts get `MACOSX_DEPLOYMENT_TARGET` and its per-OS siblings from the cc toolchain's `-target` triple, so cc-rs does not add an SDK-derived `-mmacosx-version-min=` that clashes with it under `-Werror` (drop when a rules_rust release has it). Consumers get `common:macos --action_env=MACOSX_DEPLOYMENT_TARGET=14.0` from the dx preset. Consumer fix: #11399 |
| `rules_rust` | 0.75.0 | `crate_universe_module_labels.patch` | Root only. Crate hub lockfiles pin the root module's own labels as `@@<module>+//`, so the pinned lockfile resolves when rules_dx is a dependency; the root build reads them back as `@@//` |
| `rules_rust` | 0.75.0 | `windows_exec_foreign_target_cc_linker.patch` | Root only. A generated `rust_toolchain` with a Windows exec triple and a non-Windows target triple sets `linker_preference = "cc"`, so those targets link through the cc toolchain; one with a non-Windows exec triple, an MSVC target triple and `rust-lld` sets `linker_preference = "rust"`, so those targets link with `rust-lld` as `lld-link` |
| `rules_rust` | 0.75.0 | `msvc_stdlib_ntdll.patch` | The MSVC stdlib link flags that a Rust static library passes to its C and C++ dependents include `ntdll.lib` and `kernel32.lib`, so a `cc_test` linking a Rust `staticlib` resolves the `Nt*` imports of `std` (bazelbuild/rules_rust#4376; drop when a rules_rust release has it). Consumer fix: #11399 |
| `rules_rust` | 0.75.0 | `rust_analyzer_rustc_lib.patch` | Root only. The rust-analyzer toolchain carries `rustc_lib`, and `rust_analyzer_toolchain_alias` puts it in its runfiles, because `rust-analyzer` loads `librustc_driver` from `$ORIGIN/../lib`; without it the binary fails to start when only declared inputs of the toolchain repository are on disk |
| `rules_rust_wasm_bindgen` | 0.75.0 | `ring_clang_cl_aarch64_windows.patch` | The vendored `ring` 0.17.14 gets `patches/ring/clang_cl_aarch64_windows.patch`, so the `wasm-bindgen` build of ring on Windows on Arm compiles with the hermetic clang-cl instead of looking for `clang` on `PATH` (briansmith/ring#2803; drop when a rules_rust_wasm_bindgen release vendors a ring with it). Consumer fix: #11399 |
| `rules_android` | 0.8.1 | `bundle_keystore_properties.patch` | App Bundles with key rotation: write the oldest-signer properties file instead of reading the removed `_bundle_keystore_properties` attribute. Consumer fix: #6943 |
| `rules_android` | 0.8.1 | `java8_dex_runfiles.patch` | `build_java8_legacy_dex.sh` sets `JAVA_RUNFILES` to its own runfiles tree, as `d8_desugar.sh` does, so the `d8`, `r8` and `tracereferences` launchers it resolves through the runfiles manifest find their jars when the genrule runs unsandboxed. Consumer fix: #11387 |
| `rules_android` | 0.8.1 | `public_bzl_library.patch` | Root only. `//rules:bzl` is public and `//android` has a public `bzl` `bzl_library` for `rules.bzl`, for `starlark_doc_extract` deps (drop when a rules_android release has it) |
| `rules_kotlin` | 2.4.21 | `kotlin_filegroup_android.patch` | Root only. The public `kotlin` filegroup includes `android.bzl`, for `starlark_doc_extract` deps (drop when a rules_kotlin release has it) |
| `rules_android` | 0.8.1 | `r8_globals_deterministic.patch` | Root only. The `desugar_globals` and `desugar_globals_classes` genrules run the global synthetics generator with `-Dcom.android.tools.r8.deterministicdebugging=1`, so the zip entry order is the same on every run |
| `grpc` | 1.85.1 | `pip_parse_no_310.patch` | Root only. grpc's pip hub skips python 3.10, which has no aarch64-pc-windows-msvc runtime, so windows_arm64 lock refresh evaluates |
| `grpc` | 1.85.1 | `upb_reflection_cc.patch` | Root only. Targets that depend on `@com_google_protobuf//upb/reflection` also depend on `upb/reflection:reflection_cc`, which has `def.hpp` and `message.hpp` from protobuf 37.0 on |
| `grpc` | 1.85.1 | `register_toolchains_dev.patch` | Root only. grpc registers its iOS simulator Python toolchain only when grpc is the root module, so toolchain resolution in rules_dx does not fetch the grpc repository |
| `protoc-gen-validate` | 1.2.1.bcr.2 | `pip_parse_no_39_310.patch` | Root only. pgv's pip hub skips python 3.9 and 3.10, which have no aarch64-pc-windows-msvc runtime |
| `rules_dart` | 0.10.0 | `windows_long_dill.patch` | Root only. On Windows `test_runner.dart` resolves the symlinked kernel snapshot before starting the test |
| `rules_dart` | 0.10.0 | `windows_copy_links.patch` | On Windows the module worker copies each declared input from its link target, because Dart's Windows file copy refuses a link as its source. Consumer fix: #11387 |
| `rules_dart` | 0.10.0 | `cross_revision.patch` | A cross `dart compile` reads the SDK's `revision` even when `sdk_cache` has the target binaries, so it is an `aot_compile` input and the compile works where only declared inputs exist, such as remote execution. Consumer fix: #11387 |
| `rules_java` | 9.9.0 | `toolchains_gate.patch` | Root only. The default Java toolchain definitions have `target_settings = ["@@//config:toolchains_on", ...]` |
| `rules_flutter` | 0.2.0 | `lock_extension.patch` | Root only. The `flutter` extension is not reproducible, so a lockfile written on one host does not pin another |
| `rules_flutter` | 0.2.0 | `windows_golden_comparator.patch` | `flutter_test` stages the golden comparator as a copy, so a Windows kernel worker under `--remote_download_minimal` finds it on disk. Consumer fix: #11387 |
| `rules_flutter` | 0.2.0 | `flutter_extension_loads.patch` | The `flutter` extension loads `DEFAULT_SDK_NAME` from `flutter/private/default_sdk.bzl` instead of `pub_extension.bzl`, so evaluating it loads no Apple, Swift or Android rules. Consumer fix: #11387 |
| `rules_flutter` | 0.2.0 | `host_independent_extension.patch` | Root only. The `flutter` extension reads no host: the web SDK and toolchains repositories read the machine when they are fetched, so the one lockfile entry `lock_extension.patch` records serves every host |
| `rules_perl` | 2.1.0 | `manifest_only_runfiles.patch` | The `perl_binary` entrypoint builds its runfiles directory from `RUNFILES_MANIFEST_FILE` when `RUNFILES_DIR` is set but empty, as the bash runfiles library exports it in manifest-only mode (to be upstreamed to bazel-contrib/rules_perl). Consumer fix: #11387 |
| `rules_swift` | 4.3.0 | `windows_absolute_sdk.patch` | Root only. Windows Swift actions pass `-sdk` through the wrapper as an absolute path |
| `rules_swift` | 4.3.0 | `windows_swift_testing.patch` | Root only. The Windows toolchain puts the Swift Testing module and import library directories on the compile and link paths (bazelbuild/rules_swift#2020; drop when a rules_swift release has it) |
| `rules_swift` | 4.3.0 | `windows_unknown_vendor.patch` | Root only. Windows target triples use the `unknown` vendor |
| `rules_swift` | 4.3.0 | `xcode_select_absolute_path.patch` | The `system_sdk` extension runs `/usr/bin/xcode-select`, so Xcode discovery works with no host `PATH`. Consumer fix: #11387 |
| `rules_python` | 2.4.6 | `single_import_lib.patch` | `py_extension` on Windows links one CPython import library: the stable ABI one for limited API modules, else the versioned one. It passed both as one linker argument (bazel-contrib/rules_python#4426; drop when a rules_python release has it). Consumer fix: #11418 |
| `aspect_rules_js` | 3.6.0 | `manifest_only_runfiles.patch` | Root only. The `js_binary` bash launcher accepts a `RUNFILES_MANIFEST_FILE` at any path and builds its runfiles tree from the manifest when there is no runfiles directory beside it |
| `rules_python` | 2.4.6 | `install_name_tool.patch` | Root only. The macOS `python_repository` skips the LC_ID_DYLIB fixup, which needs `install_name_tool` on PATH, when `libpython` already names itself `@rpath/` |
| `rules_python` | 2.4.6 | `lock_extension.patch` | Root only. The `python` extension is OS and architecture dependent and not reproducible, so a lockfile written on one host does not pin another |
| `rules_py` | 2.0.0-alpha.11 | `native_sdist_toolchain_flags.patch` | Native sdist builds get the cc toolchain flags (aspect-build/rules_py#1664). Consumer fix: #12898 |
| `rules_py` | 2.0.0-alpha.11 | `pep517_msvc.patch` | PEP 517 builds find the MSVC toolchain on Windows (aspect-build/rules_py#1662). Consumer fix: #12898 |
| `rules_py` | 2.0.0-alpha.11 | `venv_bin_files.patch` | The runtime sets `venv_bin_files`, so `java_binary` analyzes on Windows; there the venv's `python.exe` and DLLs are file symlinks and wheels join `sys.path` through `.pth` lines, so the launcher does not hit `CreateProcess failed` and wheel imports resolve (aspect-build/rules_py#1670). Consumer fix: #12898 |
| `rules_py` | 2.0.0-alpha.11 | `host_interpreter_windows_arm64.patch` | rules_py can see its `python_3_13_aarch64_pc_windows_msvc` interpreter, so sdist builds load on Windows ARM64 hosts (aspect-build/rules_py#1677). Consumer fix: #12898 |
| `rules_py` | 2.0.0-alpha.11 | `external_repo_pytest_paths.patch` | `py_pytest_main` finds `.pytest_paths` for targets in an external repository under `../<repo>/<package>` (aspect-build/rules_py#1679). Consumer fix: #12898 |
| `rules_py` | 2.0.0-alpha.11 | `host_libc_glibc.patch` | The uv host repo reports glibc 2.28 on Linux without running `ldd` and libsystem 14.0 on macOS without running `sw_vers`, so it fetches where neither is on PATH (aspect-build/rules_py#1686). Consumer fix: #12898 |
| `rules_py` | 2.0.0-alpha.11 | `win32_x86_32.patch` | The `win32` wheel tag matches `@platforms//cpu:x86_32`, so x86_64 Windows selects the `win_amd64` wheel (aspect-build/rules_py#1685). Consumer fix: #12898 |
| `rules_py` | 2.0.0-alpha.11 | `uv_extensions_bzl.patch` | Root only. `@rules_py//uv:extensions` is a public `bzl_library` of `uv/extensions.bzl`, so `//tools/api` extracts `//python:extensions.bzl` (upstream-ready; drop when a rules_py release has it) |
| `avro-cpp` | 1.12.0.bcr.1 | `iterator.patch` | Missing std includes (`<iterator>`, `<memory>`, `<vector>`) with libc++, added to the headers that use them; `Exception.hh` includes `fmt/format.h` for fmt 12; `avrogencpp` header guards drop the time-seeded random suffix, so two runs write the same header. Consumer fix: #6592 |
| `fmt` | 12.2.0.bcr.1 | none | Raises avro-cpp's fmt to a release with the libc++ include fix. Consumer fix: #6592 |
| `gcc_toolchain` | 0.13.0 | `sanitizer_spec.patch` | Root only. Ships `libsanitizer.spec` so GCC `-fsanitize=` links in the sandbox. Root-only: `cc/private/toolchain/gcc_repo/` vendors the patched repository rule, so consumers use BCR 0.13.0 unpatched (upstream f0rmiga/gcc-toolchain#253) |
| `gcc_toolchain` | 0.13.0 | `runtime_libs.patch` | Root only. Exposes the GCC runtime libraries (libstdc++, libgcc) to link actions. Root-only: `cc/private/toolchain/gcc_repo/` vendors the patched repository rule, so consumers use BCR 0.13.0 unpatched (upstream f0rmiga/gcc-toolchain#253) |
| `gcc_toolchain` | 0.13.0 | `target_identity_config.patch` | Root only. `cc_toolchain_config` takes `target_libc` and `target_system_name` and reports them instead of `local` (upstream: f0rmiga/gcc-toolchain#257, commit 08f057a; drop when a gcc_toolchain release has it). Root-only: `cc/private/toolchain/gcc_repo/` vendors the patched repository rule, so consumers use BCR 0.13.0 unpatched (upstream f0rmiga/gcc-toolchain#253) |
| `gcc_toolchain` | 0.13.0 | `target_identity_defs.patch` | Root only. Repository rule passes `target_libc` and `<arch>-linux-gnu` into the generated toolchain (upstream: f0rmiga/gcc-toolchain#257, commit 08f057a; drop when a gcc_toolchain release has it). Root-only: `cc/private/toolchain/gcc_repo/` vendors the patched repository rule, so consumers use BCR 0.13.0 unpatched (upstream f0rmiga/gcc-toolchain#253) |
| `gcc_toolchain` | 0.13.0 | `target_identity_extension.patch` | Root only. Module extension forwards `target_libc` and the target system name to the repository rule (upstream: f0rmiga/gcc-toolchain#257, commit 08f057a; drop when a gcc_toolchain release has it). Root-only: `cc/private/toolchain/gcc_repo/` vendors the patched repository rule, so consumers use BCR 0.13.0 unpatched (upstream f0rmiga/gcc-toolchain#253) |
| `gcc_toolchain` | 0.13.0 | `native_include_layout.patch` | Root only. `cxx_builtin_includes` lists the C++ include dirs from `include/c++` unless the toolchain ships `<target>-linux/include/c++`, on every target arch. Root-only: `cc/private/toolchain/gcc_repo/` vendors the patched repository rule, so consumers use BCR 0.13.0 unpatched (upstream f0rmiga/gcc-toolchain#253) |
| `llvm` | 0.8.27 | `libunwind_copy.patch` | Root only. `libunwind.a` is a real copy of the runtime archive, not a symlink, so a build whose `bazel-out` was deleted while the action cache was kept does not fail with `libunwind.a is a dangling symbolic link`. The cause is a Bazel bug (bazelbuild/bazel#32451) that hits every symlink output; consumers get the fix from the Bazel release that has it, and rules_dx drops the patch when `.bazelversion` reaches that release. |
| `llvm` | 0.8.27 | `clang_cl_profile_no_mingw.patch` | `compiler-rt:profile` takes no mingw headers on the windows msvc ABI, so clang-cl's `stdint.h` `#include_next` finds the MSVC CRT (upstream: hermeticbuild/hermetic-llvm#970; drop when an llvm release has it). Consumer fix: #11416 |
| `llvm` | 0.8.27 | `unwatched_overlay_inputs.patch` | Root only. `llvm-project` watches its patch and overlay tree once instead of recording each file, so a warm build reaches the repo contents cache hit in a few steps, not 37. Bazel fixes this for every module (bazelbuild/bazel#32483, cherry-pick bazelbuild/bazel#32781); consumers get the fix from the Bazel release that has it, and rules_dx drops the patch when `.bazelversion` reaches that release. |
| `llvm` | 0.8.27 | `lse_atomic_sources.patch` | The aarch64 builtins' `lse_<pat>_<size>_<model>.S` copies of `lse.S` are written when `llvm-project` is fetched, not 125 symlink actions per configuration (upstream: hermeticbuild/hermetic-llvm#922; drop when an llvm release has it). Consumer fix: #11416 |
| `llvm` | 0.8.27 | `msvc_source_directories.patch` | The MSVC header and library subsets that clang-cl searches are aliases to directories `@msvc_runtime` writes when fetched, not `copy_to_directory` actions (upstream: hermeticbuild/hermetic-llvm#919; drop when an llvm release has it). Consumer fix: #11416 |
| `llvm` | 0.8.27 | `stage0_runtime_features.patch` | Stage0 runtime objects reset the same features as the bootstrap transition, so they share one configuration: `//cli/process/...` configures about 71k targets on Linux instead of 84k (upstream: hermeticbuild/hermetic-llvm#925; drop when an llvm release has it). Consumer fix: #11416 |
| `llvm` | 0.8.27 | `macos_sdk_resolve_symlinks.patch` | The macOS SDK repo copies file symlinks, drops dangling ones, and flattens each `.framework` to its `Versions/Current` contents when fetched, so the sysroot has no symlinks and each header has one path; each framework keeps its `.tbd` at `Versions/<version>/` so ld64.lld resolves re-exports by install name (upstream: hermeticbuild/hermetic-llvm#932; drop when an llvm release has it). Consumer fix: #11416 |
| `llvm` | 0.8.27 | `msvc_fuzzer.patch` | The MSVC ABI accepts the `fuzzer` feature and stages `clang_rt.fuzzer*`; compiler-rt builds libFuzzer against libc++ headers without mingw headers; `asan` stays unsupported (upstream: hermeticbuild/hermetic-llvm#971; drop when an llvm release has it). Consumer fix: #11416 |
| `llvm` | 0.8.27 | `msvc_coverage.patch` | The MSVC ABI accepts `coverage` and `llvm_coverage_map_format`, compiles with `-fprofile-instr-generate -fcoverage-mapping`, and stages `clang_rt.profile` under coverage; `gcc_coverage_map_format` stays unsupported (upstream: hermeticbuild/hermetic-llvm#975; drop when an llvm release has it). Consumer fix: #11416 |
| `rules_foreign_cc` | 0.17.0 | `cmake_4_1_to_4_5.patch` | Root only. `CMAKE_MINORS` and `cmake_versions.bzl` list CMake 4.1 to 4.5 as upstream's generator writes them, which no rules_foreign_cc release has yet (drop when a rules_foreign_cc release has it) |
| `windows_support` | 0.4.1 | `host_tools.patch` | Root only. Opt-in `host_tools` tag keeps the MSVC compiler tools and Windows SDK `bin` (upstream: hermeticbuild/windows_support#19, which now proposes an opt-in `include_binaries`). Consumers get this patch through `@dx_windows_support`, which the `cc` extension loads; the root override serves `@llvm`'s `@msvc_runtime` and `@windows_sdk`. |
| `windows_support` | 0.4.1 | `rules_dx_hosts.patch` | Root only. `host_tools` is on by default under `rules_dx`, so `@dx_msvc` has `cl.exe`; child names match case-insensitively, since payload directory case differs from `Hostx64`. Consumers get this patch through `@dx_windows_support`, which the `cc` extension loads; the root override serves `@llvm`'s `@msvc_runtime` and `@windows_sdk`. |
| `windows_support` | 0.4.1 | `dx_msvc_layout.patch` | Root only. `@msvc_runtime` writes the MSVC header and library subsets that llvm's clang-cl toolchain searches and exposes them as `subdirectory` targets (upstream: hermeticbuild/windows_support#20 and PR #21; drop when a windows_support release has it). Consumers get this patch through `@dx_windows_support`, which the `cc` extension loads; the root override serves `@llvm`'s `@msvc_runtime` and `@windows_sdk`. |
| `windows_support` | 0.4.1 | `source_directory_headers.patch` | Root only. The Windows SDK `ucrt`, `shared`, `um` and `winrt` include targets give compiles one source directory each instead of about 4,000 loose headers, so Bazel caches their Merkle trees (upstream: hermeticbuild/windows_support#22 and PR #23; drop when a windows_support release has it). Consumers get this patch through `@dx_windows_support`, which the `cc` extension loads; the root override serves `@llvm`'s `@msvc_runtime` and `@windows_sdk`. |
| `windows_support` | 0.4.1 | `hostarm64_packages.patch` | Root only. `@msvc_runtime` selects the `Hostarm64` MSVC packages and their dependencies when `host_tools` is set. Consumers get this patch through `@dx_windows_support`, which the `cc` extension loads; the root override serves `@llvm`'s `@msvc_runtime` and `@windows_sdk`. |
| `llvm` | 0.8.27 | `mingw_coff_no_timestamp.patch` | Compiles on the mingw ABI pass `-mno-incremental-linker-compatible`, so each COFF object has a zero `TimeDateStamp` and two cold builds write the same objects and archives (drop when an llvm release has it). Consumer fix: #11416 |
| `llvm` | 0.8.27 | `internal_symbolizer_linkmode.patch` | The sanitizer internal symbolizer and the LLVM libraries under it build once, not once for the static and once for the shared sanitizer runtimes, since the runtime linkmode no longer reaches them (drop when an llvm release has it). Consumer fix: #11416 |
| `llvm` | 0.8.27 | `runtime_config_reset.patch` | Root only. The runtimes build in one configuration whatever the coverage settings of the targets that reach them, and the crt and other stage0 objects build once for the static and the shared runtimes, since the bootstrap transition pins the runtime linkmode (drop when an llvm release has it) |
| `llvm` | 0.8.27 | `runtime_go_race_reset.patch` | Root only. The runtimes build once, not again under a `go_test` with `race = "on"`, since the runtime transitions reset the rules_go race settings. Stays rules_dx only: it names rules_go settings by their canonical labels |
| `windows_support` | 0.4.1 | `reproducible_extensions.patch` | Root only. `msvc_runtime` and `windows_sdk` are reproducible when their payloads are pinned by integrity, so `MODULE.bazel.lock` does not record them |
| `windows_support` | 0.4.1 | `dx_cc_extension.patch` | Root only. `msvc_runtime.bzl` and `windows_sdk.bzl` export the repository functions and configure attributes that the `cc` extension calls |
| `windows_support` | 0.4.1 | `license_ids.patch` | Root only. The `msvc_runtime` repository fails unless the accepted license ids file, from the `license` tag or the `cc` extension, lists `visual-studio-build-tools`, and reads no environment variable |
| `windows_support` | 0.4.1 | `pins.patch` | Root only. `msvc_runtime` reads the Visual Studio installer manifest URL and integrity from the `pins` file when it is set |
| `apple_support` | 2.12.0 | `gcov_gcno_file.patch` | `GCOV_GCNO_FILE` is set only when `gcov_gcno_file` is available, so `cc_common.get_environment_variables` without it (rules_go `cgo_context_data`, rules_rust build scripts) does not fail analysis on macOS under coverage (bazelbuild/apple_support#693, merged after 2.12.0; drop when an apple_support release has it). Consumer fix: #14200 |
| `apple_support` | 2.12.0 | `toolchains_gate.patch` | Root only. The Apple C++ toolchains have `target_settings = ["@@//config:toolchains_on"]` |
| `apple_support` | 2.12.0 | `xcode_absolute_tools.patch` | Xcode and C++ toolchain discovery run `/usr/bin/env` and `/usr/bin/xcrun`, so they work with no host `PATH` (bazelbuild/apple_support#695, merged after 2.12.0; drop when an apple_support release has it). Consumer fix: #13359 |
| `rules_apple` | 5.4.0 | `llvm_coverage_map.patch` | Root only. Under coverage, `register_binary_linking_action` requests `llvm_coverage_map_format` and disables `gcc_coverage_map_format`, as rules_swift does for its binaries, so bundles with Swift deps link the clang profile runtime (drop when a rules_apple release has it). Consumer fix: #11387 |
| `elfutils` | 0.195 | `musl.patch` | Root only. libdwfl compiles without argp, fts and GNU `strerror_r` on musl, so `//tools/coverage/kcov:kcov_static` links fully static |
| `elfutils` | 0.195 | `aarch64_config.patch` | Root only. `lib/config.h` sets `HAVE_SYS_USER_REGS` on aarch64 Linux and `HAVE_USER_PACK_MASK` when the kernel headers are Linux 5.0 or later, so the aarch64 backend compiles (drop when an elfutils BCR overlay has it) |
| `rules_go` | 0.65.0 | `lazy_nogo.patch` | Toolchain registration does not load the Go rules, so builds without Go targets never fetch `@io_bazel_rules_nogo`. Consumer fix: #11418 |
| `rules_go` | 0.65.0 | `lock_extension.patch` | Root only. The `go_sdk` extension is not reproducible, so the lockfile keeps its result per platform |
| `rules_oci` | 2.3.0 | `descriptor_image.patch` | On a Windows exec platform each layer descriptor action of `oci_image` gets its own `.bat` wrapper and lists `descriptor.sh` as a tool. Consumer fix: #11387 |
| `rules_oci` | 2.3.0 | `descriptor_util.patch` | The Windows `.bat` wrapper takes a name and calls a script outside its own directory by its execroot path. Consumer fix: #11387 |
| `rules_oci` | 2.3.0 | `image_index.patch` | `oci_image_index` wraps its shell launcher in a `.bat` on a Windows exec platform. Consumer fix: #11387 |
| `rules_oci` | 2.3.0 | `image_index_blob.patch` | `oci_image_index` skips blobs already copied from a sibling image. Consumer fix: #11387 |
| `rules_oci` | 2.3.0 | `image_layout.patch` | `oci_image` writes the base layout's `oci-layout` as a fresh file so regctl can rewrite it on Windows. Consumer fix: #11387 |

## Patches in rules_dx repository rules: elvis, rules_cuda, rules_d, rules_dotnet, rules_elixir, rules_erlang, rules_fortran, rules_haskell, rules_julia, rules_ocaml, rules_powershell, rules_ruby, rules_scala, windows_support

rules_dx applies these patches in its own repository rules, so consumers get them too and you need to do nothing: `lazy_format.patch` (elvis); `coverage_keep.patch`, `objdir_as_tempdir.patch`, `toolchain_type_alias.patch` (rules_cuda); `toolchain_type_alias.patch` (rules_d); `toolchain_type_alias.patch` (rules_fortran); `bootstrap_module.patch`, `toolchains_gate.patch`, `target_runtime.patch`, `fsharp_pdb_path.patch` (rules_dotnet); `coverage_wrapper_no_report.patch`, `hermetic_haskell.patch`, `cabal_rules.patch`, `cabal_mingw_transition.patch`, `cc_wrapper_windows_exe.patch`, `cc_toolchain_config_info.patch`, `cabal_llvm_ar.patch`, `cc_wrapper_darwin_ffi.patch`, `cabal_binary_dep_data.patch`, `cabal_static_exe.patch`, `cabal_ghcversion_relpath.patch`, `cc_wrapper_origin_rpath.patch`, `ghc_xcode_env.patch`, `llvm_backend.patch`, `toolchain_type_alias.patch`, `cabal_windows_relocatable.patch` (rules_haskell); `elixir_app_deps.patch`, `elixir_app_name.patch`, `relocatable_tree.patch`, `warnings_as_errors.patch`, `windows_ex_unit_test.patch`, `toolchain_type_alias.patch`, `relative_beam_paths.patch` (rules_elixir); `01_xrl_yrl.patch`, `02_tcw_tags.patch`, `03_auto_sname.patch`, `04_otp_tree.patch`, `05_windows_eunit_cover.patch`, `06_coverage_enabled.patch`, `07_windows_ct_cover.patch`, `08_nif_profile_per_library.patch`, `toolchain_type_alias.patch` (rules_erlang); `config_base_toml.patch`, `in_process_resolve_test.patch`, `locate_installed_package.patch`, `package_annotation_version.patch`, `template_action_input.patch`, `test_tmpdir_runfiles.patch`, `julia_1_13.patch`, `stable_precompile_paths.patch`, `fixed_heap_addresses.patch`, `stable_sysimage_paths.patch` (rules_julia); `empty_glob.patch`, `hermetic_ocaml.patch`, `native_cc_env.patch`, `platform_alias.patch`, `static_cc_deps.patch`, `toolchain_type_alias.patch` (rules_ocaml); `inline_runfiles_sh.patch` (rules_powershell); `coverage.patch`, `rubyinstaller_archive.patch`, `toolchains_gate.patch`, `toolchains_hub.patch` (rules_ruby); `dx_bzl_srcs.patch`, `manifest_only_wrapper.patch`, `protobuf_repo_name.patch`, `toolchain_type_alias.patch`, `windows_coverage_launcher.patch` (rules_scala).

## julia

`MODULE.bazel` patches the Julia 1.13.4 source archive for `//julia/private/runtime:libjulia_internal`: `deterministic_objectid.patch` gives a mutable object outside any image an `objectid` from a counter while Julia writes an image. `deterministic_image_fields.patch` takes a module's `build_id` from creation order, name and parent while Julia writes an image, zeroes the padding of a new `Core.Binding`, and keeps inference timings out of the image.

## kcov

`MODULE.bazel` patches the kcov v43 archive for `//tools/coverage/kcov`: `aslr.patch` keeps running when ASLR cannot be disabled, `elf_checksum_lock.patch` loads ELF sections before the checksum so elfutils built with locks does not deadlock, and `no_curl.patch` drops the libcurl dependency.

## libfuzzer_dotnet

`macos_section.patch` puts the extra counters of `libfuzzer-dotnet.cc` in `__DATA,__libfuzz_extra` on macOS. `snprintf.patch` replaces the deprecated `sprintf` call with `snprintf`. Both apply to the `dx_libfuzzer_dotnet` archive.

## capnproto

`dynamic_cstddef.patch` and `repo_relative_labels.patch` apply to the `dx_capnproto` source archive. `repo_relative_labels.patch` replaces the `@capnp-cpp//` labels in the Bazel files with repository-relative ones.

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
