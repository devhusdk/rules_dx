# OCaml

Build, run and test OCaml with a pinned OCaml 5.5.2 from conda-forge.
Programs compile to bytecode by default and run on the pinned `ocamlrun`;
`--@rules_dx//ocaml:emitter=sys` builds native executables instead
(see [Native code](#native-code)). Nothing is
installed on the host and nothing is fetched until a target needs it.
rules_dx registers the toolchain, so there is no setup in `MODULE.bazel`.

## Rules

Load the rules from `@rules_dx//ocaml:defs.bzl`. They wrap
[rules_ocaml](https://github.com/obazl/rules_ocaml) and take the same
attributes.

```python
load("@rules_dx//ocaml:defs.bzl", "ocaml_binary", "ocaml_library", "ocaml_module", "ocaml_signature", "ocaml_test")

ocaml_module(
    name = "stock",
    struct = "stock.ml",
)

ocaml_signature(
    name = "ledger_sig",
    src = "ledger.mli",
)

ocaml_module(
    name = "ledger",
    sig = ":ledger_sig",
    struct = "ledger.ml",
    deps = [":stock"],
)

ocaml_library(
    name = "inventory",
    manifest = [
        ":ledger",
        ":stock",
    ],
)

ocaml_module(
    name = "main",
    struct = "main.ml",
    deps = [":inventory"],
)

ocaml_binary(
    name = "stockctl",
    main = ":main",
)

ocaml_module(
    name = "ledger_test_main",
    testonly = True,
    struct = "ledger_test.ml",
    deps = [":inventory"],
)

ocaml_test(
    name = "ledger_test",
    main = ":ledger_test_main",
)
```

| Rule | What it makes |
| --- | --- |
| `ocaml_module` | One compiled module from `struct` and an optional `sig` |
| `ocaml_signature` | One compiled interface from an `.mli` file |
| `ocaml_library` | Modules grouped under one label through `manifest` |
| `ocaml_binary` | Runs the bytecode of `main` on `ocamlrun` with `bazel run` |
| `ocaml_test` | Runs the bytecode of `main` as a test; a non-zero exit fails it |

- A module is named after its target, capitalized. A module with an
  `ocaml_signature` in `sig` is named after the `.mli` file.
- `ocamllex` is re-exported unchanged, because lexers need it to build.
- Modules and signatures compile with `-bin-annot` (opt out with
  `-no-bin-annot`) and expose their `.cmt` and `.cmti` files through
  `OCamlCmtInfo` (`@rules_dx//ocaml:defs.bzl`) and the `cmt` and `cmti`
  output groups.
- Modules and signatures warn on unused variables and values (`-w +26+27+32..39`)
  unless their opts set `-w`.
- Data files of a binary or test are in its runfiles.

## Native code

`--@rules_dx//ocaml:emitter=sys` switches every OCaml target to the
native `ocamlopt` of the pinned install. The default, `vm`, builds bytecode.
`ocaml_binary` and `ocaml_test` run the native executable directly.

`ocamlopt` assembles and links with the rules_dx LLVM C toolchain (clang,
LLD, `llvm-ar`), the same one `cc_library` uses. The toolchain sets the
`CONDA_OCAML_*` variables to those tools, so no host `as`, `cc` or `ld` runs.
Only `bash` and the other tools in `/usr/bin` and `/bin` that the conda
wrapper scripts need come from the host.

C stubs are `cc_library` targets named in `cc_deps` of the module that
declares the `external`:

```python
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
C stubs are not supported in the bytecode cell.

Native builds need a patch to rules_ocaml, `patches/rules_ocaml/native_cc_env.patch`.
It passes the toolchain `cc_link_env_vars` and the C toolchain files to the
compiler actions and changes nothing when the field is empty, so bytecode
actions are unchanged.

## Example

See [ocaml-app](tests/fixtures/ocaml-app/README.md) for a multi-package app with a pinned `Astring` dependency, an ocamllex lexer and a C stub that only the native cell builds.

## Formatting and linting

`dx format` runs `topiary` on `.ml` and `.mli` sources, and `dx lint` runs
`zanuda`, by default: the default policy selects `format = ["topiary"]` and
`lint = ["typos", "zanuda"]` for the `ocaml` family. zanuda has a build for
linux_x86_64, linux_arm64 and macos_arm64 and skips with a note elsewhere. topiary's output differs from ocamlformat and
a `.ocamlformat` file is ignored. `.mll` and `.mly` files are not formatted.
`dx typecheck` replays each
`ocaml_module` and `ocaml_signature` compile with `-stop-after typing` when the
`ocaml` family selects `typecheck = ["ocaml"]`. The replay writes only to a
scratch directory. Modules that use a ppx rewriter are not checked.

### Dune libraries

`dx_dune_library` (`@rules_dx//ocaml:defs.bzl`) runs `dune build -p <package> @install`
on a dune workspace and returns the package as rules_ocaml providers, so
`ocaml_module` and `ocaml_test` take it in `deps`. The package is copied to the
tree artifact `<name>_ocamlpath/<package>` (a findlib layout); the archive named
by `library` (`.cma`, or `.cmxa` plus `.a` under the native emitter) is exposed
as its own file. `dune` is a label, for example `@rules_dx//ocaml:dune`.
`deps` names other `dx_dune_library` targets; their trees go on `OCAMLPATH`.
A package with `lib<library>_stubs.a` needs `stubs = True`, which links the
archive natively (the bytecode cell does not support C stubs). See
`ocaml/tests/fixtures/dune-lib`.

### Coverage with your own ppx

Under `bazel coverage` on Linux x86_64, Linux arm64 and macOS arm64,
`ocaml_module` runs the bisect_ppx instrumenter on hand-written `.ml` files. A module that sets its own `ppx` is
left alone, because rules_ocaml takes one ppx executable per module. To cover
such a module, link `@rules_dx//ocaml:bisect_ppx_rewriter` (the
bisect_ppx rewriter as a library) into your ppxlib driver and set
`coverage_ppx = True`. Under coverage the macro then adds only
`-loc-filename <package>/<file>` and the `BISECT_RUNTIME` dependency.

### Internal tool: zanuda

`//ocaml/private/quality:zanuda` is the zanuda 2.1.0 linter, built from source
and wired into `dx lint` (#4621) as the `zanuda` lint tool of the `ocaml` family.
The lint aspect reads the typed trees (`.cmt`/`.cmti`) that `ocaml_module` and
`ocaml_signature` expose through `OCamlCmtInfo`, so a target compiled with
`-no-bin-annot` fails closed. A source-alias patch makes typed and untyped
findings report the same workspace path.

- **Build.** `dx_dune_build` (`ocaml/private/dune.bzl`) builds zanuda with the
  shared `//ocaml/private/toolchain:dune` bootstrap (`dx_dune_bootstrap`) using the pinned native OCaml,
  the resolved C toolchain, no network and no dune cache. The 22 source
  archives are pinned in `core/quality/artifacts/zanuda.bzl` and unpacked side by
  side by `@dx_ocaml_tools_duniverse`. The rule is generic over sources and target.
- **Patches.** `ocaml/private/quality/patches/` removes zanuda's dynamic plugin
  loading, adds `-cmt-list FILE` (analyze listed sources and `.cmt` files
  without loading a dune project, and exit non-zero for a skipped or
  unreadable file), makes ppx_expect_nobase's version check static, and adds
  dune files to mtime. The first two are meant for upstream.
- **Run time.** zanuda type checks against the compiler's standard library, so
  set `OCAMLLIB` to the toolchain's `lib/ocaml`. It always exits 0 on findings
  and writes them to `-ordjsonl FILE`; `-version` prints `2.0.0` because the
  upstream dune-project still says so.
- **License.** zanuda is LGPL-3.0-only and runs as a separate process; the
  licenses of the whole closure are in `//ocaml/private/quality:zanuda_licenses`.
- **Updating.** Change the pins in `zanuda.bzl` and keep the OCaml version inside
  `ZANUDA_OCAML_RANGES`; `pins_test` fails if the toolchain leaves the range,
  because `.cmt` files only load with the compiler that wrote them.

## Opam packages

`ocaml.lock(name, lock)` reads the pinned versions from the `depends:` list
of a `.opam.locked` file and creates a hub with one alias per package. Sources
and sha256 digests (sha512 when an opam file has no sha256) come from the opam-repository snapshot pinned in
`ocaml/private/toolchain/pins.json`; nothing runs opam. Compiler and `base-*` packages
and `dune` come from the toolchain and get no alias. A package without an
`ocaml.annotation` is a dune package: it is built with `dx_dune_library` and
`//ocaml:dune` (dune 3.24.2 from `ocaml/private/toolchain/pins.json`), and its
library is the package name with `-` and `.` as `_`. Set
`ocaml.annotation(package, library = ...)` for another library name. A dune
package that depends on another non-toolchain package fails until dune package
dependencies are supported. A package that is not built with dune needs
`ocaml.annotation(package, build_file, strip_prefix)`.

`@dx_ocaml_toolchains//:unix` and `:str` are the compiler's own `unix` and
`str` libraries, for `deps` of modules and of packages built by hand.

### Fuzz tests with Crowbar

`ocaml_binary` and `ocaml_test` take modules through `deps` and `main`, not `srcs`.

`ocaml_fuzz_test` builds the module `main` that registers
[Crowbar](https://github.com/stedolan/crowbar) tests and runs it in quickcheck
mode as a test. Crowbar's flags, such as `-r 5000` or `-s <seed>`, go in `args`.
The target is `manual`, tagged `dx-fuzz`, and takes the `ocaml_test` size and
timeout. `<name>_run` runs `-s <seed> -r 500` rounds with fresh seeds until
`--time` is up; a failing seed is written to `crash-<hash>` and replays with
`--repro`. Opam sets crowbar, cmdliner and afl-persistent with the BUILD files
in `@rules_dx//ocaml/packages`; `{hub}` in them becomes the hub name.

```python
# MODULE.bazel
ocaml = use_extension("@rules_dx//ocaml:extensions.bzl", "ocaml")
ocaml.lock(name = "ocaml_fuzz", lock = "//:ocaml_fuzz.opam.locked")
ocaml.annotation(
    package = "afl-persistent",
    build_file = "@rules_dx//ocaml/packages:afl-persistent.BUILD.bazel",
    strip_prefix = "ocaml-afl-persistent-1.4",
)
ocaml.annotation(package = "cmdliner", build_file = "@rules_dx//ocaml/packages:cmdliner.BUILD.bazel")
ocaml.annotation(package = "crowbar", build_file = "@rules_dx//ocaml/packages:crowbar.BUILD.bazel")
use_repo(ocaml, "ocaml_fuzz")

# BUILD.bazel
ocaml_fuzz_test(
    name = "reverse_fuzz_test",
    main = "reverse_fuzz.ml",
    deps = ["@ocaml_fuzz//:crowbar"],
)
```

### Benchmarks

`ocaml_bench(name, struct, deps)` builds the module `struct` and runs it as the
manual `dx-bench` test `dx bench` runs. `struct` calls `Dx_bench.run` from
`//ocaml/bench:dx_bench` with a list of named jobs. Each job runs once to warm
up, then in 10 timed samples of `iterations` calls (default 1000), and the
results go to `$TEST_UNDECLARED_OUTPUTS_DIR/dx-bench.json` in ns/iter.

```python
load("@rules_dx//ocaml:defs.bzl", "ocaml_bench")

ocaml_bench(
    name = "sum_bench",
    struct = "sum_bench.ml",
)
```

```ocaml
let () = Dx_bench.run [ ("sum", fun () -> ignore (Sys.opaque_identity (1 + 2))) ]
```

The benchmark always builds native code, whatever the emitter flag says.

## Platforms

| Cell | Emitter | Status |
| --- | --- | --- |
| `linux_x86_64` | bytecode (`vm`) | Qualified |
| `linux_x86_64` | native (`sys`) | Tested, not yet qualified |
| `linux_arm64` | any | Not qualified |
| `macos_x86_64` | any | Not qualified |
| `macos_arm64` | any | Not qualified |
| `windows_x86_64` | any | Not applicable (conda-forge win-64 OCaml stops at 5.3.0, below the pinned 5.5.2) |
| `windows_arm64` | n/a | conda-forge ships no win-arm64 OCaml package |

On other platforms OCaml targets fail toolchain resolution.

## API docs

`ocaml_doc_shard` (`@rules_dx//tools/docs/ocaml:defs.bzl`) turns an `ocaml_module`, `ocaml_signature` or `ocaml_library` into a DocIr shard with the pinned odoc 3.2.2, built from the tools duniverse on linux_x86_64, linux_arm64 and macos_arm64. Other platforms skip OCaml in the docs aspect. The `config` attribute is the identity recorded on every symbol as `ocaml.config`; it defaults to `odoc=3.2.2`.
