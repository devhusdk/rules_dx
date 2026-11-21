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

Add the package to the root `package.json`, then run `dx generate --apply`.
dx writes `pnpm-lock.yaml` if it is missing. An import of a package listed in
the root `package.json` resolves to `//:node_modules/<package>`. Imports that
match `paths` in the nearest `tsconfig.json` resolve to local targets.

### Writing targets

`typescript_binary` compiles `srcs` and runs the compiled `main`. `main`
defaults to the only source, and must be a `.ts`, `.mts` or `.cts` file in
`srcs`. The binary also takes `deps`, `data`, `args`, `env`, `tsconfig`,
`transpiler` and `declaration`. Do not set `entry_point`: it comes from
`main`, and setting it fails.

`typescript_test` runs `jest` by default and needs `node_modules`. To run the
file with Node instead, set `runner = "node"` and a `main`. On Windows, Jest
coverage uses the v8 provider.

## Quality

### Format, lint and typecheck

`biome` formats and lints TypeScript, and `tsc` checks types. Biome reads
`biome.json` at the workspace root.

```sh
dx format --apply
dx lint
dx typecheck
```

To lint with `eslint` too, put `eslint.config.mjs` at the root and bind it
with `eslint_config`.

### Browser tests

`typescript_playwright_test` runs Playwright tests in a pinned headless
Chromium. Bazel downloads the browser and the Linux libraries it loads, so
you install nothing. Add `@playwright/test` 1.65.0 to the root
`package.json`. With another version, the test fails and names the version to
install.

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

The test sets `DX_PLAYWRIGHT_APP` to the `app` binary. Have the app listen on
port 0 and print its URL, such as `listening on http://127.0.0.1:41234`. The
config starts the app, waits for that line and uses the URL:

```ts
export default defineConfig({
  use: { baseURL: process.env.DX_PLAYWRIGHT_URL, screenshot: "only-on-failure" },
  webServer: {
    command: process.env.DX_PLAYWRIGHT_APP ?? "",
    wait: { stdout: /listening on (?<dx_playwright_url>http:\/\/\S+)/ },
  },
});
```

`dx test` reads the JUnit `test.xml`. Traces and screenshots of failed tests
go to `playwright/` in the undeclared outputs. `dx test --output=json` lists
the outputs zip as `outputs_path` on each failed `test_outcome`.

The test runs on Linux x86_64, Linux arm64 and macOS arm64. Other platforms
skip it.

## Bench and fuzz

`typescript_bench` runs a tinybench `Bench` for `dx bench`.
`typescript_fuzz_test` runs a Jazzer.js target for `dx fuzz`. List
`tinybench` or `@jazzer.js/core` in `package.json` and pass `node_modules`.
See [`dx bench`](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/bench.md)
and [`dx fuzz`](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/fuzz.md).

## Platforms

See the [support matrix](https://github.com/devhusdk/rules_dx/blob/main/docs/support-matrix.md).
