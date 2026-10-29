# OCaml

Build, run and test OCaml with a pinned OCaml 5.5.2 from conda-forge.
Programs compile to bytecode by default and run on the pinned `ocamlrun`;
`--@rules_dx//ocaml/toolchain:emitter=sys` builds native executables instead
(see [Native code](#native-code)). Nothing is
installed on the host and nothing is fetched until a target needs it.
rules_dx registers the toolchain, so there is no setup in `MODULE.bazel`.

## Rules

Load the rules from `@rules_dx//ocaml/rules:defs.bzl`. They wrap
[rules_ocaml](https://github.com/obazl/rules_ocaml) and take the same
attributes.

```python
load("@rules_dx//ocaml/rules:defs.bzl", "ocaml_binary", "ocaml_library", "ocaml_module", "ocaml_signature", "ocaml_test")

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
- `ocamllex` and `ocamlyacc` are re-exported unchanged.
- Modules and signatures compile with `-bin-annot` (opt out with
  `-no-bin-annot`) and expose their `.cmt` and `.cmti` files through
  `OCamlCmtInfo` (`//ocaml/rules:providers.bzl`) and the `cmt` and `cmti`
  output groups.
- Data files of a binary or test are in its runfiles.

## Native code

`--@rules_dx//ocaml/toolchain:emitter=sys` switches every OCaml target to the
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

See [examples/ocaml-app](../examples/ocaml-app/README.md) for a multi-package app with a pinned `Astring` dependency, an ocamllex lexer and a C stub that only the native cell builds.

## Formatting and linting

`dx format` runs `topiary` on `.ml` and `.mli` sources when the `ocaml` family
selects it with `format = ["topiary"]`. Its output differs from ocamlformat and
a `.ocamlformat` file is ignored. `.mll` and `.mly` files are not formatted. No
OCaml linter runs. The rules tag every target `no-lint` and `no-typecheck`, so
`dx lint` and `dx typecheck` skip it. The compiler is the type check.

### Internal tool: zanuda

`//quality/tools/ocaml:zanuda` is the zanuda 2.1.0 linter, built from source
for the future `lint:ocaml` support (#4621). Nothing runs it yet; the lint
aspect, adapter and runner dispatch come next.

- **Build.** `dx_dune_build` (`quality/tools/ocaml/dune_build.bzl`) bootstraps
  dune and builds zanuda in two Bazel actions with the pinned native OCaml,
  the resolved C toolchain, no network and no dune cache. The 22 source
  archives are pinned in `quality/tools/ocaml/pins.bzl` and unpacked side by
  side by `@dx_ocaml_duniverse`. The rule is generic over sources and target.
- **Patches.** `quality/tools/ocaml/patches/` removes zanuda's dynamic plugin
  loading, adds `-cmt-list FILE` (analyze listed sources and `.cmt` files
  without loading a dune project, and exit non-zero for a skipped or
  unreadable file), makes ppx_expect_nobase's version check static, and adds
  dune files to mtime. The first two are meant for upstream.
- **Run time.** zanuda type checks against the compiler's standard library, so
  set `OCAMLLIB` to the toolchain's `lib/ocaml`. It always exits 0 on findings
  and writes them to `-ordjsonl FILE`; `-version` prints `2.0.0` because the
  upstream dune-project still says so.
- **License.** zanuda is LGPL-3.0-only and runs as a separate process; the
  licenses of the whole closure are in `//quality/tools/ocaml:zanuda_licenses`.
- **Updating.** Change the pins in `pins.bzl` and keep the OCaml version inside
  `ZANUDA_OCAML_RANGES`; `pins_test` fails if the toolchain leaves the range,
  because `.cmt` files only load with the compiler that wrote them.

## Platforms

| Cell | Emitter | Status |
| --- | --- | --- |
| `linux_x86_64` | bytecode (`vm`) | Qualified |
| `linux_x86_64` | native (`sys`) | Tested, not yet qualified |
| `linux_arm64` | any | Not qualified |
| `macos_x86_64` | any | Not qualified |
| `macos_arm64` | any | Not qualified |
| `windows_x86_64` | any | Not qualified |

On other platforms OCaml targets fail toolchain resolution.
