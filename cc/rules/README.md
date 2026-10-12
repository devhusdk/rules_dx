# C/C++ rules

`defs.bzl` wraps `cc_library`, `cc_binary` and `cc_test`.

```starlark
load("@rules_dx//cc/rules:defs.bzl", "cc_library", "cc_test")
```

Loading the wrappers registers no toolchain. Opt into the hermetic defaults with the `MODULE.bazel.snippet` that `dx init --apply` writes. The snippet pins `rules_cc`, `llvm`, `rules_rust` and registers `@llvm//toolchain:all` and `@rust_toolchains//:all`. Keep the `bazel_dep` lines and drop the `register_toolchains` lines to use your own toolchains. Native host builds are the default. The default Windows toolchain uses the GNU ABI. MSVC ABI selection stays a manual step.

## Options

`copts`, `defines`, `features`, `includes` and `linkopts` reach the upstream rule exactly as written. A `select()` value stays a `select()` value. The requested language standard is never translated: `-std=c++23` compiles as C++23.

The wrappers add no warning flags.

## Warnings

Warnings are the toolchain's decision. Repository builds enable the upstream `treat_warnings_as_errors` feature in `.bazelrc`. A consumer selects it on its own toolchain:

```sh
bazel build --features=treat_warnings_as_errors //pkg/...
```

The flags are driver syntax. A GCC or Clang toolchain takes `-Wall -Werror`. An MSVC-style driver takes `/W4 /WX`. Plain C never receives a C++-only option.