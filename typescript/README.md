# TypeScript

This directory helps you build, run and test TypeScript code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//typescript:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [TypeScript page](../docs/languages/typescript.md) shows
setup, every rule and the tools dx runs.

Run the TypeScript tests from a rules_dx checkout:

```sh
tools/dx test //typescript/...
```

## Dependencies

Add the package to the root `package.json`, then run `dx generate --apply`. dx
writes a missing `pnpm-lock.yaml`. An import of a package listed
in the root `package.json` resolves to `//:node_modules/<package>`. Imports
that match `paths` in the nearest `tsconfig.json` resolve to local targets.

### Writing targets

`typescript_binary` compiles `srcs` and runs the compiled `main`, which
defaults to the only source. `main` must be a `.ts`, `.mts` or `.cts` file
listed in `srcs`. It also takes `deps`, `data`, `args`, `env`, `tsconfig`,
`transpiler` and `declaration`. `entry_point` comes from `main`; setting it
fails.

`typescript_test` runs `jest` by default and needs `node_modules`. Set
`runner = "node"` with a `main` to run the file with Node instead.

On Windows, Jest coverage uses the v8 provider.

## Quality

### Browser tests

`typescript_playwright_test` runs Playwright tests in a pinned headless Chromium. The
browser and the Linux libraries it loads are downloaded by Bazel, so none has to be
installed. Add `@playwright/test` 1.65.0 to the root `package.json`. Another version
fails the test with the version to install.

```python
load("@rules_dx//typescript:defs.bzl", "typescript_playwright_test")

typescript_playwright_test(
    name = "app_e2e_test",
    srcs = ["app.spec.ts"],
    app = ":server",
    config = "playwright.config.ts",
    node_modules = "//:node_modules",
)
```

The test sets `DX_PLAYWRIGHT_PORT` to a free port and `DX_PLAYWRIGHT_APP` to the
`app` binary. Start the app from the config:

```ts
const port = Number(process.env.DX_PLAYWRIGHT_PORT);

export default defineConfig({
  use: { baseURL: `http://127.0.0.1:${port}`, screenshot: "only-on-failure" },
  webServer: { command: process.env.DX_PLAYWRIGHT_APP ?? "", port },
});
```

`dx test` reads the JUnit `test.xml`. Traces and screenshots of failed tests are
written to `playwright/` in the undeclared outputs. `dx test --output=json` lists
the outputs zip as `outputs_path` on each failed `test_outcome`.
Supported platforms are Linux x86_64, Linux arm64 and macOS arm64. On any other
platform the test is skipped.

### Format, lint and typecheck

`biome` formats and lints TypeScript, and `tsc` checks types. Biome reads
`biome.json` at the workspace root. To lint with `eslint`, put
`eslint.config.mjs` at the root and bind it with `eslint_config`.

```sh
dx format --apply
dx lint
dx typecheck
```

## Bench and fuzz

`typescript_bench` runs a tinybench `Bench` for `dx bench`, and `typescript_fuzz_test` runs a Jazzer.js target for `dx fuzz`. List `tinybench` or `@jazzer.js/core` in `package.json` and pass `node_modules`. See [`dx bench`](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/bench.md) and [`dx fuzz`](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/fuzz.md).

## Platforms

See the [support matrix](https://github.com/devhusdk/rules_dx/blob/main/docs/support-matrix.md).
