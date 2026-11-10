# OCaml App

A multi-package OCaml stock tool. It reads a ledger file, prints a report and
a checksum of the report, and reports bad input with distinct exit codes.

| Package | What it shows |
| --- | --- |
| `lexer` | `tokens.mll` goes through `ocamllex` into a module (generated code) |
| `ledger` | A module with an `.mli` that uses the pinned `Astring` library and the lexer |
| `checksum` | One interface, two implementations chosen by a `select()` on the emitter flag |
| `checksum/native` | The C-backed implementation, built only in the native cell |
| `checksum/portable` | The same algorithm in plain OCaml |
| `cli` | The `stockctl` binary and an exit-code test |
| `diagnostics` | A file the compiler must reject |

## Run it

```sh
bazel test //ocaml/tests/fixtures/ocaml-app/...
bazel run //ocaml/tests/fixtures/ocaml-app/cli:stockctl -- ocaml/tests/fixtures/ocaml-app/cli/good.txt
```

The default cell compiles to bytecode and runs on the pinned `ocamlrun`.
Pass `--@rules_dx//ocaml:emitter=sys` to build native executables.
The same flag picks the checksum module: the native cell links the C stub in
`checksum/native`, the bytecode cell uses `checksum/portable`, because the
bytecode cell links no C code. Both must pass the same test vectors.

```sh
bazel test //ocaml/tests/fixtures/ocaml-app/... --@rules_dx//ocaml:emitter=sys
```

## Exit codes

| Code | Meaning |
| --- | --- |
| 0 | The report was printed |
| 2 | A malformed line, or a wrong command line |
| 4 | A line holds more than 1000 units |

`//ocaml/tests/fixtures/ocaml-app/cli:exit_codes_test` checks each code.

## Pinned dependency

`Astring` 0.8.5 is listed in `ocaml_app.opam.locked`; `ocaml.lock` in `MODULE.bazel`
takes its archive and `sha256` from the pinned opam-repository snapshot. Its
ISC license applies. `astring.BUILD.bazel` builds the seven
library modules one by one in the order of upstream's `astring.mllib` and
leaves out the toplevel printers. Warnings are off for that third-party code
only.

## Negative cases

| Target | Message |
| --- | --- |
| `//ocaml/tests/fixtures/ocaml-app/cli:report_without_astring` | `Unbound module Astring` |
| `//ocaml/tests/fixtures/ocaml-app/diagnostics:syntax_error` | `Syntax error` |

Both are `manual`, so a wildcard build skips them.

## Platforms

The example is qualified on `linux_x86_64` with the bytecode cell. The native
cell is tested in the repository through `//ocaml/tests/native`, but it is not
yet qualified in a hermetic consumer.
