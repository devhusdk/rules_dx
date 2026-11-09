# Haskell

The `rules_dx_haskell` module registers the Haskell toolchain, as a
`bazel_dep` next to `rules_dx`. A workspace with Haskell targets adds
`bazel_dep(name = "rules_dx_haskell")`. The rules, the tests and the quality
tools stay in `rules_dx`, because its own tool targets use them.
