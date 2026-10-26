# Fortran

Build, run and test Fortran with a pinned static Flang 21.1.8. The Fortran
runtime, flang-rt 21.1.8, is built from source with the same C++ toolchain as
the rest of the build, and binaries and tests link through `cc_binary` and
`cc_test`. Nothing is installed on the host and nothing is fetched until a
target needs it. rules_dx registers the toolchain, so there is no setup in
`MODULE.bazel`.

## Start

```python
load("@rules_dx//fortran/rules:defs.bzl", "fortran_binary", "fortran_library", "fortran_test")

fortran_library(
    name = "units",
    srcs = ["units.f90"],
)

fortran_library(
    name = "solver",
    srcs = ["solver.f90"],
    deps = [":units"],
)

fortran_binary(
    name = "solve",
    srcs = ["main.f90"],
    deps = [":solver"],
)

fortran_test(
    name = "solver_test",
    srcs = ["solver_test.f90"],
    deps = [":solver"],
)
```

```sh
bazel run //:solve
bazel test //:solver_test
```

## Rules

Load the rules from `@rules_dx//fortran/rules:defs.bzl`. `fortran_library`
wraps the [rules_fortran](https://github.com/miinso/rules_fortran) rule and
takes the same attributes.

| Rule | What it makes |
| --- | --- |
| `fortran_library` | A static library and the `.mod` files of its modules |
| `fortran_binary` | A program linked by `cc_binary`; run it with `bazel run` |
| `fortran_test` | A program linked by `cc_test`; a non-zero exit or `error stop` fails it |

`fortran_binary` and `fortran_test` take two kinds of attributes:

| Goes to | Attributes |
| --- | --- |
| The Fortran compile | `srcs`, `deps`, `copts`, `defines`, `hdrs`, `includes` |
| The link and the run | `linkopts`, `data`, `args`, `env`, `env_inherit`, `size`, `timeout`, `stamp` and the other `cc_binary` or `cc_test` attributes |

- Sources end in `.f`, `.f90`, `.f95`, `.f03` or `.f08`. Uppercase
  extensions (`.F`, `.F90`, ...) run the preprocessor first; `defines` only
  work on those.
- `.f` is fixed form. The others are free form.
- `linkopts` on a `fortran_library` only reach other Fortran targets. Put
  link options on the `fortran_binary` or `fortran_test`.

## Modules

A source can `use` modules from the targets in its `deps`. It cannot `use` a
module from another source of the same target. Put each module that other
code uses in its own `fortran_library` and share it through `deps`.

The intrinsic modules (`iso_fortran_env`, `iso_c_binding`, `ieee_*`) work
without any dependency.

## C interop

A `fortran_library` gives C and C++ rules a `CcInfo`, and it can depend on a
`cc_library`.

- Fortran calling C: declare an `interface` with `bind(c)` and add the
  `cc_library` to `deps`.
- C calling Fortran: write the routine with `bind(c)` and add the
  `fortran_library` to the `deps` of a `cc_binary` or `cc_test`. The runtime
  comes with it.

## Formatting and linting

No Fortran formatter or linter runs. The rules tag every target `no-format`,
`no-lint` and `no-typecheck`, so `dx format`, `dx lint` and `dx typecheck`
skip it. The compiler is the type check.

## Platforms

| Cell | Status |
| --- | --- |
| `linux_x86_64` | Qualified |
| `linux_arm64` | Unsupported |
| `macos_arm64` | Unsupported |
| `windows_x86_64` | Unsupported |
| `windows_arm64` | Unsupported |

OpenMP, GPU offload and Wasm are unsupported. On other platforms Fortran
targets are incompatible and skipped.
