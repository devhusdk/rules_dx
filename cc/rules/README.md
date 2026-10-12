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

## Windows ABI

Windows builds default to the MinGW ABI. Nothing infers MSVC from the operating system alone.

Select the native MSVC ABI explicitly per build:

```sh
bazel build \
  --platforms=@llvm//platforms:windows_x86_64_msvc \
  --repo_env=BAZEL_MSVC_RUNTIME_VISUAL_STUDIO_EULA=1 \
  --repo_env=BAZEL_WINDOWS_SDK_EULA=1 \
  //pkg/...
```

The two `repo_env` flags record acceptance of the Microsoft Visual C++ runtime and Windows SDK license terms. The repository never sets them; each user accepts them for their own builds. Without them the build fails during analysis and names the missing flag.

The MSVC platform compiles with clang-cl for `x86_64-pc-windows-msvc`, links with lld-link against the Microsoft runtime and SDK with a statically built libc++, and defaults to the retail dynamic CRT (`/MD`). The linked binary imports only `KERNEL32`, `VCRUNTIME140` and the UCRT API sets; no MinGW runtime or flags leak in. It runs on native Windows without a manual runtime install.

Rust targets do not resolve for the MSVC platform, so mixed Rust/C++ binaries are not qualified there yet.