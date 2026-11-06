# JVM

Java, Kotlin and Scala tests and fixtures. They live in the `rules_dx_jvm` module, a
`bazel_dep` next to `rules_dx`. The rules, the toolchains and the quality tools stay
in `rules_dx`, because its own tool targets use them.

- [Java](java/README.md)
- [Kotlin](kotlin/README.md)
- [Scala](scala/README.md)

The module pins its test artifacts in its own `@dx_jvm_maven` repository, with the lock in
`third_party/jvm/dx_jvm_maven_install.json`. Inside the module it is visible as `@maven`, so
fixture labels read `@maven//:...`; a consumer's `@maven` never sees these artifacts. Repin
with `RULES_JVM_EXTERNAL_REPIN=1 bazel run @maven//:pin` from `families/jvm`.
