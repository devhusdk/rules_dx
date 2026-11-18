# C and C++

This directory helps you build, run and test C and C++ code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//cc:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [C and C++ page](../docs/languages/cc.md) shows
setup, every rule and the tools dx runs.

Run the C and C++ tests from a rules_dx checkout:

```sh
tools/dx test //cc/...
```

## Dependencies

### CMake dependencies

`cmake_library` from `@rules_dx//cmake:defs.bzl` only formats and lints CMake files. It never builds them.

To build a CMake project, use the rules_foreign_cc `cmake` rule and register `tools.explicit()` with a pinned cmake and ninja, so no host cmake is used.

```starlark
cmake(
    name = "zlib_static",
    generate_args = ["-GNinja"],
    lib_source = "@zlib//:all_srcs",
    out_static_libs = ["libz.a"],
)

cc_binary(
    name = "app",
    srcs = ["main.c"],
    deps = [":zlib_static"],
)
```

This is tested on Linux x86_64 with LLVM and GCC, with LLVM cross-compiling to Linux arm64, and on Windows x64 with MSVC (cl.exe through the Ninja generator). On Linux, `bash` is the only host tool. On Windows, ninja is a pinned prebuilt binary.

## Quality

### Warnings

Each wrapper adds the upstream `treat_warnings_as_errors` feature to `features`. Set `warnings_as_errors = False` on a target to leave it out, or `True` to keep it whatever the flag says. Without the attribute, the target follows `//config:warnings_as_errors`, which is on by default. To turn it off for the whole repository:

```sh
common --@rules_dx//config:warnings_as_errors=false
```

The flags are driver syntax. A GCC or Clang toolchain takes `-Wall -Werror`. An MSVC-style driver takes `/W4 /WX`. Plain C never receives a C++-only option.

## Bench and fuzz

`cc_bench` links Google Benchmark's `benchmark_main` into a private `<name>_bin` and declares `<name>` as the `dx-bench` test `dx bench` runs. Test attributes (`size`, `timeout`, `args`, `data`, `env`) go to `<name>`, the other attributes to the binary.

```starlark
load("@rules_dx//cc:defs.bzl", "cc_bench")

cc_bench(
    name = "parse_bench",
    srcs = ["parse_bench.cc"],
)
```

The test writes `dx-bench.json` with one row per benchmark, using the real time in the unit the benchmark reports. Google Benchmark comes from the `google_benchmark` Bazel module that rules_dx depends on.

## API docs

`dx docs` documents the public `hdrs` of a `cc_library` with the pinned Doxygen, with no setup. Declare `cc_doc_shard(name = "docs", lib = ":lib")` from `@rules_dx//tools/docs/cc:defs.bzl` to set the package name. Doxygen publishes no Linux arm64 or Windows arm64 build, so those hosts get no C or C++ API docs.

## Toolchain

### Options

`copts`, `defines`, `features`, `includes` and `linkopts` reach the upstream rule exactly as written. A `select()` value stays a `select()` value. The requested language standard is never translated: `-std=c++23` compiles as C++23.

### GCC

To use your own compiler instead, see [Use your own C/C++ toolchain](../docs/own-cc-toolchain.md).

Select the GCC toolchain in `.bazelrc`:

```text
common --@rules_dx//cc:compiler=gcc
```

Or per build:

```sh
bazel build --@rules_dx//cc:compiler=gcc //pkg/...
```

It targets Linux x86_64 with GCC 16.2.0 and a glibc 2.26 floor. libstdc++ and libgcc link statically, so each shared library carries its own copy. Do not pass `std::` types across a `.so` boundary. Other targets fail at analysis instead of falling back to LLVM.

The `dynamic_libstdcxx` feature links the toolchain's `libstdc++.so.6` and `libgcc_s.so.1` instead, ships both in runfiles and sets an `$ORIGIN` rpath, so nothing loads from the host. It covers dynamically linked targets (`cc_test`, `linkstatic = False`); a statically linked `cc_binary` keeps the static runtime.

```sh
bazel test --@rules_dx//cc:compiler=gcc --features=dynamic_libstdcxx //pkg:test
```

`cc_test` coverage works with both toolchains. GCC coverage works on Linux x86_64 with the gcov that ships with the toolchain, and collection fails when gcov reports a version, stamp or profile mismatch, corrupt data or a missing notes file. MSVC/Windows coverage is not supported. `//tools/coverage/collect:gcov_native_test` runs the collector on a GCC-instrumented binary.

Sanitizers work under GCC with the `asan` and `ubsan` features. The toolchain links the runtimes statically, so nothing loads from the host. GCC's static libasan still fails its link order check at startup, so `cc_test` sets `ASAN_OPTIONS=verify_asan_link_order=0` under GCC unless the target's `env` sets `ASAN_OPTIONS`. A `cc_binary` run with `bazel run` still needs it set by hand. The asan command:

```sh
bazel test --@rules_dx//cc:compiler=gcc --features=asan //pkg:test
bazel test --@rules_dx//cc:compiler=gcc --features=ubsan //pkg:test
```

On a macOS host, `--features=asan`, `ubsan` or `tsan` uses the Xcode toolchain, and the runtimes load from the installed Xcode, so this is not hermetic.

### MSVC

Select the MSVC toolchain in `.bazelrc`:

```text
common --@rules_dx//cc:compiler=msvc
```

It supports a Windows x86_64 host that targets Windows x86_64. Accept the license in the root `MODULE.bazel`:

```text
dx_licenses = use_extension("@rules_dx//core:extensions.bzl", "dx_licenses")
dx_licenses.accept(ids = ["visual-studio-build-tools"])
```

Any other host or target platform fails with a message that names the supported pair. There is no clang-cl fallback.

### macOS

On a macOS host, Apple targets including `macos_*` resolve the Xcode toolchain; `@llvm` with the pinned `@macos_sdk` serves macOS targets only from Linux and Windows hosts. The dx preset sets macOS 14.0 as the minimum for target and host builds; override it with `--macos_minimum_os`, `--host_macos_minimum_os` and the matching `MACOSX_DEPLOYMENT_TARGET` in `--action_env` and `--host_action_env`.

### Objective-C

`cc_objc_library` is the upstream rule under your name. rules_apple resources
and frameworks, and rules_swift `module_name`, behave exactly as upstream.
dx assigns `srcs`, `non_arc_srcs` and `hdrs` to quality checks by their paths.
