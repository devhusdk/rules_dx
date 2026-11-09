# Fortran

The `rules_dx_fortran` module registers the Fortran toolchain, as a
`bazel_dep` next to `rules_dx`. A workspace with Fortran targets adds
`bazel_dep(name = "rules_dx_fortran")`. The rules, the tests and the quality
tools stay in `rules_dx`, because its own tool targets use them.
