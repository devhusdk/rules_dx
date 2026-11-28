# Adopt Scala example

Scala sbt-layout tree with a `greet` package and a stdlib-only `solo` package.

```sh
bazel run @rules_dx//dx/launch:dx -- init //scala/tests/fixtures/adopt_scala/...
bazel run @rules_dx//dx/launch:dx -- generate //scala/tests/fixtures/adopt_scala/...
bazel build //scala/tests/fixtures/adopt_scala/...
bazel test //scala/tests/fixtures/adopt_scala/...
```

Both tests pass (`greet_test`, `pure_test`).

For Scalafix lint, see `docs/cli/commands/quality.md`.
