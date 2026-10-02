# Adopt JS/TS example

JS/TS tree with an `app` package and a `web` TypeScript package.

```sh
bazel run //cli/cli:dx -- init //examples/adopt-js-ts/...
bazel run //cli/cli:dx -- generate //examples/adopt-js-ts/...
bazel build //examples/adopt-js-ts/...
bazel test //examples/adopt-js-ts/...
```

Both tests pass (`greet_test`, `app_test`). The TypeScript wrapper emits its
own typecheck tests, so `bazel test` runs more targets than the example
declares.
