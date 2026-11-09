# Swift

The `rules_dx_swift` module registers the Swift toolchain, as a
`bazel_dep` next to `rules_dx`. A workspace with Swift targets adds
`bazel_dep(name = "rules_dx_swift")`. The rules, the tests and the quality
tools stay in `rules_dx`, because its own tool targets use them.
