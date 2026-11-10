# C/C++ rules

`defs.bzl` wraps `cc_library`, `cc_binary`, `cc_test` and `objc_library`.

```starlark
load("@rules_dx//cc/rules:defs.bzl", "cc_binary", "cc_library", "cc_test", "objc_library")

cc_library(
    name = "greet",
    srcs = ["greet.cc"],
    hdrs = ["greet.h"],
)

cc_binary(
    name = "hello",
    srcs = ["main.cc"],
    deps = [":greet"],
)

cc_test(
    name = "greet_test",
    srcs = ["greet_test.cc"],
    deps = [":greet"],
)
```

`objc_library` is the upstream rule under your name, with no forwarder. rules_apple resources and frameworks, and rules_swift `module_name`, behave exactly as upstream. Quality ownership of `srcs`, `non_arc_srcs` and `hdrs` comes from the target's paths.

## CMake dependencies

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

## Options

`copts`, `defines`, `features`, `includes` and `linkopts` reach the upstream rule exactly as written. A `select()` value stays a `select()` value. The requested language standard is never translated: `-std=c++23` compiles as C++23.

## Warnings

Each wrapper adds the upstream `treat_warnings_as_errors` feature to `features`. Set `warnings_as_errors = False` on a target to leave it out, or `True` to keep it whatever the flag says. Without the attribute, the target follows `//config:warnings_as_errors`, which is on by default. To turn it off for the whole repository:

```sh
common --@rules_dx//config:warnings_as_errors=false
```

The flags are driver syntax. A GCC or Clang toolchain takes `-Wall -Werror`. An MSVC-style driver takes `/W4 /WX`. Plain C never receives a C++-only option.

## GCC

To use your own compiler instead, see [Use your own C/C++ toolchain](../../docs/own-cc-toolchain.md).

Select the GCC toolchain in `.bazelrc`:

```text
common --@rules_dx//cc/toolchains:compiler=gcc
```

Or per build:

```sh
bazel build --@rules_dx//cc/toolchains:compiler=gcc //pkg/...
```

It targets Linux x86_64 with GCC 16.2.0 and a glibc 2.26 floor. libstdc++ and libgcc link statically, so each shared library carries its own copy. Do not pass `std::` types across a `.so` boundary. Other targets fail at analysis instead of falling back to LLVM.

The `dynamic_libstdcxx` feature links the toolchain's `libstdc++.so.6` and `libgcc_s.so.1` instead, ships both in runfiles and sets an `$ORIGIN` rpath, so nothing loads from the host. It covers dynamically linked targets (`cc_test`, `linkstatic = False`); a statically linked `cc_binary` keeps the static runtime.

```sh
bazel test --@rules_dx//cc/toolchains:compiler=gcc --features=dynamic_libstdcxx //pkg:test
```

`cc_test` coverage works with both toolchains. GCC coverage works on Linux x86_64 with the gcov that ships with the toolchain, and collection fails when gcov reports a version, stamp or profile mismatch, corrupt data or a missing notes file. MSVC/Windows coverage is not supported. `//cc/toolchains:gcc_coverage_env_test` checks that wiring, and `//tools/coverage/collect:gcov_native_test` runs the collector on a GCC-instrumented binary.

Sanitizers work under GCC with the `asan` and `ubsan` features. The toolchain links the runtimes statically, so nothing loads from the host. GCC's static libasan still fails its link order check at startup (#5981), so `cc_test` sets `ASAN_OPTIONS=verify_asan_link_order=0` under GCC unless the target's `env` sets `ASAN_OPTIONS`. A `cc_binary` run with `bazel run` still needs it set by hand. The asan command:

```sh
bazel test --@rules_dx//cc/toolchains:compiler=gcc --features=asan //pkg:test
bazel test --@rules_dx//cc/toolchains:compiler=gcc --features=ubsan //pkg:test
```

## MSVC

Select the MSVC toolchain in `.bazelrc`:

```text
common --@rules_dx//cc/toolchains:compiler=msvc
```

It supports a Windows x86_64 host that targets Windows x86_64. Accept the license in the root `MODULE.bazel`:

```text
msvc = use_extension("@rules_dx//cc/toolchains/msvc:extensions.bzl", "msvc")
msvc.license(accept = ["visual-studio-build-tools"])
```

Any other host or target platform fails with a message that names the supported pair. There is no clang-cl fallback.

The hermetic harness denies the preinstalled Visual Studio, Windows Kits and Microsoft SDKs directories when it qualifies an MSVC consumer, so a build that reaches them fails. `c_fixture_msvc` and `cpp_fixture_msvc` qualified that way on `windows_x86_64` in ci.yml [run 42372644803](https://github.com/devhusdk/rules_dx/actions/runs/42372644803). They qualified the same way on `windows_arm64` in ci.yml [run 42375312749](https://github.com/devhusdk/rules_dx/actions/runs/42375312749). On `windows_arm64`, `msvc_arm64_hello_test` and `msvc_arm64_pe_machine_test` passed in [run 42183869292](https://github.com/devhusdk/rules_dx/actions/runs/42183869292).

## Benchmarks

`cc_bench` links Google Benchmark's `benchmark_main` into a private `<name>_bin` and declares `<name>` as the manual `dx-bench` test `dx bench` runs. Test attributes (`size`, `timeout`, `args`, `data`, `env`) go to `<name>`, the other attributes to the binary.

```starlark
load("@rules_dx//cc/rules:defs.bzl", "cc_bench")

cc_bench(
    name = "parse_bench",
    srcs = ["parse_bench.cc"],
)
```

The test writes `dx-bench.json` with one row per benchmark, using the real time in the unit the benchmark reports. Google Benchmark comes from the `google_benchmark` Bazel module that rules_dx depends on.

## API docs

`dx docs` documents the public `hdrs` of a `cc_library` with the pinned Doxygen, with no setup. Declare `cc_doc_shard(name = "docs", lib = ":lib")` from `@rules_dx//tools/docs/cc:defs.bzl` to set the package name. Doxygen publishes no Linux arm64 or Windows arm64 build, so those hosts get no C or C++ API docs.
