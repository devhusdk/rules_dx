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
- Data files of a binary or test are in its runfiles.

## Native code

`--@rules_dx//ocaml/toolchain:emitter=sys` switches every OCaml target to the
native `ocamlopt` of the pinned install. The default, `vm`, builds bytecode.
`ocaml_binary` and `ocaml_test` run the native executable directly.

`ocamlopt` assembles and links with the rules_dx LLVM C toolchain (clang,
LLD, `llvm-ar`), the same one `cc_library` uses. The toolchain sets the
`CONDA_OCAML_*` variables to those tools together with the C toolchain's own
compile and link flags (`--sysroot=/dev/null`, the hermetic crt objects, lld, compiler-rt), so no host `as`, `cc` or `ld` and no host sysroot is used.
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

## Formatting and linting

No OCaml formatter or linter runs. The rules tag every target `no-format`,
`no-lint` and `no-typecheck`, so `dx format`, `dx lint` and `dx typecheck`
skip it. The compiler is the type check.

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
