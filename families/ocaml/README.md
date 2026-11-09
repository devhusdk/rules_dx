# OCaml

The `rules_dx_ocaml` module registers the OCaml toolchain, as a
`bazel_dep` next to `rules_dx`. A workspace with OCaml targets adds
`bazel_dep(name = "rules_dx_ocaml")`. The rules, the tests and the quality
tools stay in `rules_dx`, because its own tool targets use them.
