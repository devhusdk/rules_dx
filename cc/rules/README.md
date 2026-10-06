# C/C++ rules

`defs.bzl` wraps `cc_library`, `cc_binary` and `cc_test`.

```starlark
load("@rules_dx//cc/rules:defs.bzl", "cc_library", "cc_test")
```

## Options

`copts`, `defines`, `features`, `includes` and `linkopts` reach the upstream rule exactly as written. A `select()` value stays a `select()` value. The requested language standard is never translated: `-std=c++23` compiles as C++23.

The wrappers add no warning flags.

## Warnings

Warnings are the toolchain's decision. Repository builds enable the upstream `treat_warnings_as_errors` feature in `.bazelrc`. A consumer selects it on its own toolchain:

```sh
bazel build --features=treat_warnings_as_errors //pkg/...
```

The flags are driver syntax. A GCC or Clang toolchain takes `-Wall -Werror`. An MSVC-style driver takes `/W4 /WX`. Plain C never receives a C++-only option.