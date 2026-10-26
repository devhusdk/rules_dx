# OCaml

Build, run and test OCaml with a pinned OCaml 5.5.2 from conda-forge.
Programs compile to bytecode and run on the pinned `ocamlrun`. Nothing is
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

## Formatting and linting

No OCaml formatter or linter runs. The rules tag every target `no-format`,
`no-lint` and `no-typecheck`, so `dx format`, `dx lint` and `dx typecheck`
skip it. The compiler is the type check.

## Platforms

| Cell | Emitter | Status |
| --- | --- | --- |
| `linux_x86_64` | bytecode (`vm`) | Qualified |
| `linux_x86_64` | native | Not qualified |
| `linux_arm64` | any | Not qualified |
| `macos_x86_64` | any | Not qualified |
| `macos_arm64` | any | Not qualified |
| `windows_x86_64` | any | Not qualified |

On other platforms OCaml targets fail toolchain resolution.
