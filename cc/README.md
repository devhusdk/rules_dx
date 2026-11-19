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

`cmake_library` from `@rules_dx//cmake:defs.bzl` only formats and lints CMake
files. It never builds them.

To build a CMake project, use the `cmake` rule from rules_foreign_cc. Add
`rules_foreign_cc` as a `bazel_dep` and pin cmake and ninja in your
`MODULE.bazel`. `tools.explicit()` turns off the fallback to a cmake
installed on the host:

```starlark
tools = use_extension("@rules_foreign_cc//foreign_cc:extensions.bzl", "tools")
tools.explicit()
tools.cmake(mode = "binary", version = "4.0.7")
tools.ninja(mode = "binary", version = "1.13.2")
```

Then build the project and link it like any other library. `lib_source` is a
filegroup with the project's sources:

```starlark
load("@rules_dx//cc:defs.bzl", "cc_binary")
load("@rules_foreign_cc//foreign_cc:defs.bzl", "cmake")

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

This works on Linux x86_64 with LLVM or GCC, from Linux x86_64 to Linux arm64
with LLVM, and on Windows x64 with MSVC through the Ninja generator. On Linux
the host needs only `bash`.

## Quality

### Warnings

Warnings fail the build by default. Each rule adds the upstream
`treat_warnings_as_errors` feature to `features`. On one target, set
`warnings_as_errors = False` to allow warnings, or `True` to always fail on
them. To allow warnings in the whole repository, add this line to `.bazelrc`,
the file of Bazel flags at the root of your repository:

```text
common --@rules_dx//config:warnings_as_errors=false
```

GCC and Clang get `-Wall -Werror`. MSVC gets `/W4 /WX`. C sources never get a
C++-only flag.

## Coverage

`bazel coverage` and `dx coverage` work for `cc_test` with LLVM and with GCC.
GCC coverage runs on Linux x86_64 with the gcov that ships with the toolchain.
It fails when gcov reports a version, stamp or profile mismatch, corrupt data
or a missing notes file. MSVC builds get no coverage.

## Bench and fuzz

`cc_bench` builds a Google Benchmark binary for `dx bench`:

```starlark
load("@rules_dx//cc:defs.bzl", "cc_bench")

cc_bench(
    name = "parse_bench",
    srcs = ["parse_bench.cc"],
)
```

It links `benchmark_main` from the `google_benchmark` module, which rules_dx
brings, so you add no `bazel_dep`. `<name>` is the test that `dx bench` runs
and `<name>_bin` is the binary. `size`, `timeout`, `args`, `data` and `env`
go to the test. The other attributes go to the binary. The test writes
`dx-bench.json` with one row per benchmark, in real time and in the unit the
benchmark reports.

## API docs

`dx docs` documents the public `hdrs` of each `cc_library` with the pinned
Doxygen. You need no setup. To set the package name, declare a shard:

```starlark
load("@rules_dx//cc:defs.bzl", "cc_doc_shard")

cc_doc_shard(
    name = "docs",
    lib = ":lib",
    package = "mylib",
)
```

Doxygen has no Linux arm64 or Windows arm64 build, so those hosts get no C or
C++ API docs.

## Toolchain

LLVM is the default compiler. To use your own compiler instead, see
[Use your own C/C++ toolchain](../docs/own-cc-toolchain.md).

### Options

`copts`, `defines`, `features`, `includes` and `linkopts` reach the compiler
exactly as you write them, `select()` values included. The language standard
is never rewritten: `-std=c++23` compiles as C++23.

### GCC

To build with GCC, add this line to `.bazelrc`:

```text
common --@rules_dx//cc:compiler=gcc
```

Or pass the flag to one build:

```sh
bazel build --@rules_dx//cc:compiler=gcc //pkg/...
```

GCC 16.2.0 builds for Linux x86_64 and Linux arm64, with a glibc 2.26 floor.
Any other target fails at analysis, the step where Bazel reads BUILD files
before it runs anything. It never falls back to LLVM.

libstdc++ and libgcc link statically, so each shared library carries its own
copy. Do not pass `std::` types across a `.so` boundary. To link them
dynamically instead, turn on the `dynamic_libstdcxx` feature:

```sh
bazel test --@rules_dx//cc:compiler=gcc --features=dynamic_libstdcxx //pkg:test
```

The feature ships the toolchain's `libstdc++.so.6` and `libgcc_s.so.1` in
runfiles, the files Bazel places next to a binary when it runs, and sets an
`$ORIGIN` rpath, so nothing loads from the host. It covers dynamically linked
targets, such as `cc_test` or a target with `linkstatic = False`. A statically
linked `cc_binary` keeps the static runtime.

### Sanitizers

With GCC, turn on AddressSanitizer or UndefinedBehaviorSanitizer with the
`asan` or `ubsan` feature:

```sh
bazel test --@rules_dx//cc:compiler=gcc --features=asan //pkg:test
bazel test --@rules_dx//cc:compiler=gcc --features=ubsan //pkg:test
```

GCC links the runtimes statically, so nothing loads from the host.
`cc_test` sets `ASAN_OPTIONS=verify_asan_link_order=0` unless the target's
`env` sets `ASAN_OPTIONS`. For a `cc_binary` you start with `bazel run`, set
it yourself:

```sh
ASAN_OPTIONS=verify_asan_link_order=0 bazel run --@rules_dx//cc:compiler=gcc --features=asan //pkg:app
```

On a macOS host, `asan`, `ubsan` and `tsan` use the Xcode toolchain, and the
runtimes load from the installed Xcode.

### MSVC

To build with MSVC, add this line to `.bazelrc`:

```text
common --@rules_dx//cc:compiler=msvc
```

MSVC builds on a Windows x86_64 host for Windows x86_64. Accept the Visual
Studio Build Tools license in your root `MODULE.bazel`:

```starlark
dx_licenses = use_extension("@rules_dx//core:extensions.bzl", "dx_licenses")
dx_licenses.accept(ids = ["visual-studio-build-tools"])
```

Any other host or target fails with a message that names the supported pair.
It never falls back to clang-cl.

### macOS

On a macOS host, Apple targets, `macos_*` included, build with the Xcode
toolchain. On Linux and Windows hosts, macOS targets build with `@llvm` and
the pinned `@macos_sdk`.

The minimum macOS version is 14.0 for target and host builds. To change it,
set all four flags in `.bazelrc`:

```text
common --macos_minimum_os=15.0
common --host_macos_minimum_os=15.0
common --action_env=MACOSX_DEPLOYMENT_TARGET=15.0
common --host_action_env=MACOSX_DEPLOYMENT_TARGET=15.0
```

### Objective-C

`cc_objc_library` is the upstream `objc_library` under a rules_dx name.
rules_apple resources and frameworks, and the rules_swift `module_name`,
work exactly as upstream. dx sorts `srcs`, `non_arc_srcs` and `hdrs` into
quality checks by their file paths.
