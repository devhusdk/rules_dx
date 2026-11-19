# OCaml

This directory helps you build, run and test OCaml code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//ocaml:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [OCaml page](../docs/languages/ocaml.md) shows
setup, every rule and the tools dx runs.

Run the OCaml tests from a rules_dx checkout:

```sh
tools/dx test //ocaml/...
```

## Dependencies

### Opam packages

Pin versions in `*.opam.locked` files beside your `*.opam` manifest. Then:

1. Run `dx update --apply`. It reads the `depends:` lists of the
   `*.opam.locked` files and writes `opam.lock.json`. Sources and sha256
   digests come from the opam-repository commit that `ocaml/pins.json` pins.
   Nothing runs opam.
2. Run `dx generate --apply`. With `opam.lock.json` beside the root
   `<name>.opam`, it adds the `opam.lock` stanza for the `@opam` hub to
   `MODULE.bazel`. A hub is one Bazel repository that holds a target for
   every locked package.
3. Depend on a package as `@opam//<package>`, for example `@opam//crowbar`.

The stanza looks like this:

```python
opam = use_extension("@rules_dx//ocaml:extensions.bzl", "opam")
opam.lock(
    name = "opam",
    lock = "//:opam.lock.json",
)
use_repo(opam, "opam")
```

The hub has one package per lock entry. Compiler packages, `base-*`
packages and `dune` come from the toolchain and get no package.

How each package builds:

- A package without an `opam.annotation` is a dune package. It builds with
  `ocaml_dune_library` and `@rules_dx//ocaml:dune`. Its library is the
  package name with `-` and `.` turned into `_`. Set
  `opam.annotation(package, library = ...)` for another library name.
- A dune package that depends on a package no `*.opam.locked` file pins
  fails.
- A package that does not build with dune needs
  `opam.annotation(package, build_file, strip_prefix)`.

`@dx_ocaml_toolchains//:unix` and `:str` are the compiler's own `unix` and
`str` libraries. Put them in the `deps` of your modules, or of packages you
build by hand.

### Dune libraries

`ocaml_dune_library` from `@rules_dx//ocaml:defs.bzl` runs
`dune build -p <package> @install` on a dune workspace. It returns the
package as rules_ocaml providers, so `ocaml_module` and `ocaml_test` take it
in `deps`:

```python
load("@rules_dx//ocaml:defs.bzl", "ocaml_dune_library")

ocaml_dune_library(
    name = "greet",
    srcs = glob(["src/*", "*.opam"]) + ["dune-project"],
    dune = "@rules_dx//ocaml:dune",
    library = "greet",
    package = "greet",
    workspace = "dune-project",
)
```

- `dune` is the dune label, for example `@rules_dx//ocaml:dune`.
- `library` names the archive to expose: `.cma`, or `.cmxa` plus `.a` under
  the native emitter.
- `deps` names other `ocaml_dune_library` targets. Their packages go on
  `OCAMLPATH`.
- Set `stubs = True` for a package with `lib<library>_stubs.a`. It links the
  archive natively; the bytecode cell does not support C stubs.

The package is copied to `<name>_ocamlpath/<package>` in findlib layout.
`ocaml/tests/fixtures/dune_lib` is a working example.

## Quality

The default policy selects these tools for the `ocaml` family:

| Command | Tool | Notes |
| --- | --- | --- |
| `dx format` | `topiary` | `.ml` and `.mli` only. A `.ocamlformat` file is ignored, and the output differs from ocamlformat. |
| `dx lint` | `typos`, `zanuda` | zanuda runs on linux_x86_64, linux_arm64 and macos_arm64 and skips with a note elsewhere. |
| `dx typecheck` | `ocaml` | Only when the family sets `typecheck = ["ocaml"]`. |

- `.mll` and `.mly` files are not formatted.
- zanuda reads the typed trees (`.cmt` and `.cmti`) of `ocaml_module` and
  `ocaml_signature`. A target compiled with `-no-bin-annot` fails the lint.
- `dx typecheck` replays each `ocaml_module` and `ocaml_signature` compile
  with `-stop-after typing` and writes only to a scratch directory. Modules
  that use a ppx rewriter are not checked.

## Coverage

On Linux x86_64, Linux arm64 and macOS arm64, `bazel coverage` runs the
bisect_ppx instrumenter on the hand-written `.ml` files of each
`ocaml_module`.

A module that sets its own `ppx` is left alone, because rules_ocaml takes one
ppx executable per module. To cover such a module:

1. Link `@rules_dx//ocaml:bisect_ppx_rewriter` into your ppxlib driver.
2. Set `coverage_ppx = True` on the module.

Under coverage, the macro then adds only `-loc-filename <package>/<file>`
and the `BISECT_RUNTIME` dependency.

## Bench and fuzz

`ocaml_binary` and `ocaml_test` take modules through `deps` and `main`, not
`srcs`. The bench and fuzz rules work the same way.

### Benchmarks

`ocaml_bench` builds the module `main` and runs it as the `dx-bench` test
that `dx bench` runs:

```python
load("@rules_dx//ocaml:defs.bzl", "ocaml_bench")

ocaml_bench(
    name = "sum_bench",
    main = "sum_bench.ml",
)
```

```ocaml
let () = Dx_bench.run [ ("sum", fun () -> ignore (Sys.opaque_identity (1 + 2))) ]
```

`Dx_bench.run` takes a list of named jobs. Each job runs once to warm up,
then in 10 timed samples of `iterations` calls (default 1000). The results
go to `$TEST_UNDECLARED_OUTPUTS_DIR/dx-bench.json` in ns/iter. A benchmark
always builds native code, whatever the emitter flag says.

### Fuzz tests with Crowbar

`ocaml_fuzz_test` builds the module `main`, which registers
[Crowbar](https://github.com/stedolan/crowbar) tests, and runs it as a test
in quickcheck mode:

```python
load("@rules_dx//ocaml:defs.bzl", "ocaml_fuzz_test")

ocaml_fuzz_test(
    name = "reverse_fuzz_test",
    main = "reverse_fuzz.ml",
    deps = ["@opam//crowbar"],
)
```

- Put Crowbar flags, such as `-r 5000` or `-s <seed>`, in `args`.
- The target is tagged `dx-fuzz` and takes the `ocaml_test` size and
  timeout.
- `<name>_run` runs `-s <seed> -r 500` rounds with fresh seeds until `--time`
  is up. A failing seed is written to `crash-<hash>` and replays with
  `--repro`.
- The opam hub builds crowbar, cmdliner and afl-persistent itself. An
  `opam.annotation` for one of them replaces the whole default, including
  `strip_prefix`.

## API docs

`ocaml_doc_shard` from `@rules_dx//ocaml:defs.bzl` turns an
`ocaml_module`, `ocaml_signature` or `ocaml_library` into a DocIr shard with
the pinned odoc 3.2.2.

- It runs on linux_x86_64, linux_arm64 and macos_arm64. Other platforms
  skip OCaml in `dx docs`.
- `config` is recorded on every symbol as `ocaml.config`. It defaults to
  `odoc=3.2.2`.

## Toolchain

### Native code

By default, OCaml targets build bytecode (`vm`). To build native code with
the pinned `ocamlopt`, pass:

```sh
bazel build //app/... --@rules_dx//ocaml:emitter=sys
```

`ocaml_binary` and `ocaml_test` then run the native executable directly.

`ocamlopt` assembles and links with the rules_dx LLVM C toolchain (clang,
LLD, `llvm-ar`), the same one `cc_library` uses. No host `as`, `cc` or `ld`
runs. Only `bash` and the tools in `/usr/bin` and `/bin` that the conda
wrapper scripts need come from the host.

### C stubs

Put C stubs in a `cc_library` and name it in `cc_deps` of the module that
declares the `external`:

```python
load("@rules_dx//cc:defs.bzl", "cc_library")
load("@rules_dx//ocaml:defs.bzl", "ocaml_module")

cc_library(
    name = "stub",
    srcs = ["stub.c"],
    hdrs = ["stub.h"],
)

ocaml_module(
    name = "binding",
    struct = "binding.ml",
    cc_deps = {":stub": "static"},
)
```

A stub whose symbol is not reachable through `cc_deps` fails at link time.
The bytecode cell does not support C stubs.

### Example

[ocaml_app](tests/fixtures/ocaml_app/README.md) is a multi-package app with
a pinned `Astring` dependency, an ocaml_lex lexer and a C stub that only the
native cell builds.

## Platforms

The [support matrix](../docs/support-matrix.md) lists the platforms. On other
platforms, OCaml targets fail toolchain resolution: Bazel finds no OCaml
toolchain for that platform.
