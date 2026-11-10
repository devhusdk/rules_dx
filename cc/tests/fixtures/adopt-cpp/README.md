# Adopt C++ example

C++ tree with a `greet` package and a stdlib-only `solo` package.

```sh
bazel run @rules_dx//:dx -- init //cc/tests/fixtures/adopt-cpp/...
bazel run @rules_dx//:dx -- generate //cc/tests/fixtures/adopt-cpp/...
bazel build //cc/tests/fixtures/adopt-cpp/...
bazel test //cc/tests/fixtures/adopt-cpp/...
```

Both tests pass (`greet_test`, `pure_test`).

`copts` reaches the compiler exactly as written, so `-std=c++17` here stays C++17. Warnings are errors because the repository selects the `treat_warnings_as_errors` toolchain feature, not because the wrapper adds a flag. The same feature can also be selected on the command line:

```sh
bazel build --features=treat_warnings_as_errors //cc/tests/fixtures/adopt-cpp/...
```

See `cc/README.md` for the wrapper contract.