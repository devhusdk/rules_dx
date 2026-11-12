# Fortran

Build, run and test Fortran with a pinned static Flang 21.1.8. The Fortran
runtime, flang-rt 21.1.8, is built from source with the same C++ toolchain as
the rest of the build, and binaries and tests link through `cc_binary` and
`cc_test`. Nothing is installed on the host and nothing is fetched until a
target needs it. rules_dx registers the toolchain, so there is no setup in
`MODULE.bazel`.

## Dependencies

### Modules

A source can `use` modules from the targets in its `deps`. It cannot `use` a
module from another source of the same target. Put each module that other
code uses in its own `fortran_library` and share it through `deps`.
Sources in different packages may share a file name, such as `kinds.f90`.

The intrinsic modules (`iso_fortran_env`, `iso_c_binding`, `ieee_*`) work
without any dependency.

### Locked packages

`fpm.lock(name, lock)` in `MODULE.bazel` reads an `fpm.lock.json` and creates
the hub repository `name` with one `fortran_library` per locked package, at
`@<name>//<package>`. Each package compiles the Fortran sources under its `src`
tree and depends on the packages its own `fpm.toml` lists. `dx update` runs
`@rules_dx//fortran/private/tools/lock`, which reads the git dependencies of the
`fpm.toml` beside the lock and of every archive they name, and writes
`fpm.lock.json`. Each dependency needs a `https://github.com/<owner>/<repo>` git
URL pinned with `tag` or `rev`; path dependencies are skipped. `dx security` and
`dx license` list the locked packages.

## Quality

`dx format` runs `ffmt` and `dx lint` runs `fortitude` by default; the default policy selects
`format = ["ffmt"]` and `lint = ["fortitude", "typos"]` for the `fortran` family. It formats `.f90`, `.f95`, `.f03`, `.F90`, `.F95` and `.fpp`
sources. Fixed-form `.f` and `.F` files and `.f08` and `.F08` files are left
alone, because ffmt would corrupt or reject them. `dx typecheck` runs `flang` only when the `fortran` family
selects it with `typecheck = ["flang"]`. `fortitude` lints free-form sources (`.f90` and the
newer extensions). Fixed-form `.f` and `.F` files are not linted, because
fortitude reads free-form source only.

## API docs

`dx docs` documents every `fortran_library` that lists its own sources, with no
setup. FORD, pinned by hash in `core/quality/tools/python/uv.lock`, parses the public
modules, types and procedures on the hermetic Python toolchain; nothing is
compiled and no host tool runs. To build the shard yourself, declare one:

```python
load("@rules_dx//tools/docs/fortran:defs.bzl", "fortran_doc_shard")

fortran_doc_shard(
    name = "units_docs",
    lib = ":units",
)
```

## Toolchain

### C interop

A `fortran_library` gives C and C++ rules a `CcInfo`, and it can depend on a
`cc_library`.

- Fortran calling C: declare an `interface` with `bind(c)` and add the
  `cc_library` to `deps`.
- C calling Fortran: write the routine with `bind(c)` and add the
  `fortran_library` to the `deps` of a `cc_binary` or `cc_test`. The runtime
  comes with it.

## Platforms

See the [support matrix](https://github.com/devhusdk/rules_dx/blob/main/docs/support-matrix.md).

OpenMP, GPU offload and Wasm are unsupported. On Windows, Fortran targets are
incompatible and skipped.
