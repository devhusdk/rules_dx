# Fortran

This directory helps you build, run and test Fortran code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//fortran:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [Fortran page](../docs/languages/fortran.md) shows
setup, every rule and the tools dx runs.

Run the Fortran tests from a rules_dx checkout:

```sh
tools/dx test //fortran/...
```

## Dependencies

### Modules

A source can `use` modules from the targets in its `deps`. It cannot `use` a
module from another source of the same target. So put each module that other
code uses in its own `fortran_library`, and share it through `deps`:

```python
load("@rules_dx//fortran:defs.bzl", "fortran_library")

fortran_library(
    name = "kinds",
    srcs = ["kinds.f90"],
)

fortran_library(
    name = "units",
    srcs = ["units.f90"],
    deps = [":kinds"],
)
```

Sources in different packages may share a file name, such as `kinds.f90`.
The intrinsic modules (`iso_fortran_env`, `iso_c_binding`, `ieee_*`) need
no dependency.

### Locked packages

List git dependencies in `fpm.toml`. Each one needs a
`https://github.com/<owner>/<repo>` URL pinned with `tag` or `rev`. Path
dependencies are skipped.

1. Run `dx update --apply`. It reads the git dependencies of `fpm.toml` and
   of every archive they name, and writes `fpm.lock.json`.
2. Call `fpm.lock(name, lock)` in `MODULE.bazel`. It creates the hub `name`:
   one Bazel repository with a `fortran_library` per locked package.
3. Depend on a package as `@<name>//<package>`.

```python
fpm = use_extension("@rules_dx//fortran:extensions.bzl", "fpm")
fpm.lock(
    name = "fpm",
    lock = "//:fpm.lock.json",
)
use_repo(fpm, "fpm")
```

Each package compiles the Fortran sources under its `src` tree and depends on
the packages its own `fpm.toml` lists. `dx security` and `dx license` list
the locked packages.

## Quality

The default policy selects these tools for the `fortran` family:

| Command | Tool | Files |
| --- | --- | --- |
| `dx format` | `ffmt` | `.f90`, `.f95`, `.f03`, `.F90`, `.F95`, `.fpp` |
| `dx lint` | `fortitude`, `typos` | free-form sources: `.f90` and newer |
| `dx typecheck` | `flang` | only when the family sets `typecheck = ["flang"]` |

- ffmt leaves fixed-form `.f` and `.F` files and `.f08` and `.F08` files
  alone, because it would corrupt or reject them.
- fortitude reads free-form source only, so it does not lint `.f` and `.F`
  files.

## API docs

`dx docs` documents every `fortran_library` that lists its own sources, with
no setup. FORD parses the public modules, types and procedures on the pinned
Python toolchain. Nothing is compiled and no host tool runs.

To build the shard yourself, declare one:

```python
load("@rules_dx//fortran:defs.bzl", "fortran_doc_shard")

fortran_doc_shard(
    name = "units_docs",
    lib = ":units",
)
```

## Toolchain

The Fortran runtime, flang-rt, is built from source with the same C++
toolchain as the rest of the build. Binaries and tests link through
`cc_binary` and `cc_test`.

### C interop

A `fortran_library` gives C and C++ rules a `CcInfo`, and it can depend on a
`cc_library`.

- Fortran calling C: declare an `interface` with `bind(c)` and add the
  `cc_library` to `deps`.
- C calling Fortran: write the routine with `bind(c)` and add the
  `fortran_library` to the `deps` of a `cc_binary` or `cc_test`. The runtime
  comes with it.

## Platforms

The [support matrix](../docs/support-matrix.md) lists the platforms. OpenMP,
GPU offload and Wasm are not supported. On Windows, Fortran targets are
incompatible and skipped.
