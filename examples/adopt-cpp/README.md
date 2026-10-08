# Adopt C++ example

C++ tree with a `greet` package and a stdlib-only `solo` package.

```sh
bazel run @rules_dx//:dx -- init //examples/adopt-cpp/...
bazel run @rules_dx//:dx -- generate //examples/adopt-cpp/...
bazel build //examples/adopt-cpp/...
bazel test //examples/adopt-cpp/...
```

Both tests pass (`greet_test`, `pure_test`).

`copts` reaches the compiler exactly as written, so `-std=c++17` here stays C++17. Warnings are errors because the repository selects the `treat_warnings_as_errors` toolchain feature, not because the wrapper adds a flag. A consumer workspace selects the same feature on its own toolchain:

```sh
bazel build --features=treat_warnings_as_errors //pkg/...
```

See `cc/rules/README.md` for the wrapper contract.