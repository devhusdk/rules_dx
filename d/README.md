# D

Build, run and test D with the pinned `rules_d` 0.12.1 and LDC 1.43.0.
Nothing needs to be installed. rules_dx registers the toolchain, so there is
no setup in `MODULE.bazel`. Linking uses the hermetic LLVM C++ toolchain.

```python
load("@rules_dx//d/rules:defs.bzl", "D_COMPATIBLE_WITH", "d_binary", "d_library", "d_test")

d_library(
    name = "greet",
    srcs = ["greet.d"],
    target_compatible_with = D_COMPATIBLE_WITH,
)

d_binary(
    name = "hello",
    srcs = ["main.d"],
    target_compatible_with = D_COMPATIBLE_WITH,
    deps = [":greet"],
)

d_test(
    name = "greet_test",
    srcs = ["greet.d"],
    target_compatible_with = D_COMPATIBLE_WITH,
)
```

- `srcs` lists the `.d` and `.di` files the target owns.
- `d_test` compiles `srcs` with `-unittest` and runs every `unittest` block.
- `deps` takes D libraries and `cc_library` targets.
- `imports`, `string_srcs`, `string_imports`, `versions`, `dopts` and
  `linkopts` pass through to `rules_d`. Files in `string_srcs` are read with
  `import("file")` and are not D sources.
- `D_COMPATIBLE_WITH` skips D targets on hosts without a D toolchain.

## Formatting

Select `dfmt` in a `d` quality family to check and fix D formatting. The D
wrappers carry the pinned dfmt, so no flag is needed. See "D formatting" in
`docs/cli/commands/quality.md`.

## Linting

Select `dscanner` in a `d` quality family to lint D sources. The D wrappers
carry the pinned D-Scanner, so no flag is needed. See "D linting" in
`docs/cli/commands/quality.md`.

## Platforms

| Host | LDC | Qualified |
| --- | --- | --- |
| Linux x86_64 | yes | yes |
| Linux arm64 | yes | no |
| macOS arm64 | yes | no |
| Windows x86_64 | yes | no |
| Windows arm64 | no | no |
