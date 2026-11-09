# JavaScript

Build, run and test JavaScript with Node.

```python
load("@rules_dx//javascript/rules:defs.bzl", "javascript_binary", "javascript_library", "javascript_test")

javascript_library(
    name = "greet",
    srcs = ["greet.js"],
)

javascript_binary(
    name = "hello",
    srcs = ["main.js"],
    data = [":greet"],
    entry_point = "main.js",
)

javascript_test(
    name = "greet_test",
    srcs = ["greet.test.js"],
    data = [":greet"],
    node_modules = "//:node_modules",
)
```

`javascript_test` runs `jest` by default and needs `node_modules`. Set `runner = "node"` with an
`main` to run the file with Node instead.
On Windows, Jest coverage uses the v8 provider.

## Benchmarks and fuzzing

`javascript_bench` runs a tinybench `Bench` for `dx bench`, and `javascript_fuzz_test` runs a Jazzer.js target for `dx fuzz`. List `tinybench` or `@jazzer.js/core` in `package.json` and pass `node_modules`. See [`dx bench`](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/bench.md) and [`dx fuzz`](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/fuzz.md).

## Third-party packages

Add the package to the root `package.json`, run `pnpm install` to update
`pnpm-lock.yaml`, then run `dx generate --apply`. An import of a package listed
in the root `package.json` resolves to `//:node_modules/<package>`.

## Format and lint

`biome` formats and lints JavaScript. Its config is `biome.json` at the
workspace root. JavaScript has no typecheck tool. To lint with `eslint`, put
`eslint.config.mjs` at the root and bind it with `eslint_config`.

```sh
dx format --apply
dx lint
```

## Platforms

See the [support matrix](https://github.com/devhusdk/rules_dx/blob/main/docs/support-matrix.md).
