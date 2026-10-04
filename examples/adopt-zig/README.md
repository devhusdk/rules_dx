# Adopt zig example

Zig library adopted without upstream changes.

```sh
bazel run //cli/cli:dx -- init //examples/adopt-zig/...
bazel run //cli/cli:dx -- generate //examples/adopt-zig/...
bazel build //examples/adopt-zig/...
```

All 1 tests pass (`greet_test`).

Zig rules take `main`, the single root source file, rather than a `srcs` list a
tool infers the root from. `greet_test` lists both sources for that reason.
