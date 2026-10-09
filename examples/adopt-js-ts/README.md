# Adopt JS/TS example

JS/TS tree with an `app` package and a `web` TypeScript package.

```sh
bazel run @rules_dx//:dx -- init //examples/adopt-js-ts/...
bazel run @rules_dx//:dx -- generate //examples/adopt-js-ts/...
bazel build //examples/adopt-js-ts/...
bazel test //examples/adopt-js-ts/...
```

All 4 tests pass (`greet_test`, `plain_test`, `app_test`, `plain_test`). The TypeScript wrapper emits its
own typecheck tests, so `bazel test` runs more targets than the example
declares.
