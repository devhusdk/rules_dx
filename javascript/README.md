# JavaScript

This directory helps you build, run and test JavaScript code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//javascript:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [JavaScript page](../docs/languages/javascript.md) shows
setup, every rule and the tools dx runs.

Run the JavaScript tests from a rules_dx checkout:

```sh
tools/dx test //javascript/...
```

## Dependencies

Add the package to the root `package.json`, then run `dx generate --apply`. dx
writes a missing `pnpm-lock.yaml`. An import of a package listed
in the root `package.json` resolves to `//:node_modules/<package>`.

### Writing targets

`javascript_test` runs `jest` by default and needs `node_modules`. Set
`runner = "node"` with a `main` to run the file with Node instead.

On Windows, Jest coverage uses the v8 provider.

## Quality

`biome` formats and lints JavaScript. Its config is `biome.json` at the
workspace root. JavaScript has no typecheck tool. To lint with `eslint`, put
`eslint.config.mjs` at the root and bind it with `eslint_config`.

```sh
dx format --apply
dx lint
```

## Bench and fuzz

`javascript_bench` runs a tinybench `Bench` for `dx bench`, and `javascript_fuzz_test` runs a Jazzer.js target for `dx fuzz`. List `tinybench` or `@jazzer.js/core` in `package.json` and pass `node_modules`. See [`dx bench`](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/bench.md) and [`dx fuzz`](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/fuzz.md).

## Platforms

See the [support matrix](https://github.com/devhusdk/rules_dx/blob/main/docs/support-matrix.md).
