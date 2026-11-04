# Adopt JS/TS example

JS/TS tree with an `app` package and a `web` TypeScript package.

```sh
bazel run @rules_dx//:dx -- init //javascript/tests/fixtures/adopt-js-ts/...
bazel run @rules_dx//:dx -- generate //javascript/tests/fixtures/adopt-js-ts/...
bazel build //javascript/tests/fixtures/adopt-js-ts/...
bazel test //javascript/tests/fixtures/adopt-js-ts/...
```

All 3 tests pass (`greet_test`, `plain_test`, `app_test`). The TypeScript wrapper emits its
own typecheck tests, so `bazel test` runs more targets than the example
declares.
