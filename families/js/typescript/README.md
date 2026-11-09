# TypeScript

Build and test TypeScript with `tsc` type checking.

```python
load("@rules_dx//typescript/rules:defs.bzl", "typescript_project", "typescript_test")

typescript_project(
    name = "greet",
    srcs = ["greet.ts"],
)

typescript_test(
    name = "greet_test",
    srcs = ["greet.test.ts"],
    deps = [":greet"],
    node_modules = "//:node_modules",
)
```

`typescript_test` runs `jest` by default and needs `node_modules`. Set `runner = "node"` with an
`main` to run the file with Node instead.
On Windows, Jest coverage uses the v8 provider.

## Browser tests

`typescript_playwright_test` runs Playwright tests in a pinned headless Chromium. The
browser is downloaded by Bazel, so none has to be installed. Add
`@playwright/test` to the root `package.json`.

```python
load("@rules_dx//typescript/rules:defs.bzl", "typescript_playwright_test")

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
the outputs zip as `outputs_path` on each failed `test_outcome`. Linux needs the system
libraries Chromium links against, such as `libnss3` and `libnspr4`.
Supported hosts are Linux x86_64, Linux arm64 and macOS arm64. Windows is not
supported.

## Benchmarks and fuzzing

`typescript_bench` runs a tinybench `Bench` for `dx bench`, and `typescript_fuzz_test` runs a Jazzer.js target for `dx fuzz`. List `tinybench` or `@jazzer.js/core` in `package.json` and pass `node_modules`. See [`dx bench`](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/bench.md) and [`dx fuzz`](https://github.com/devhusdk/rules_dx/blob/main/docs/cli/commands/fuzz.md).

## Third-party packages

Add the package to the root `package.json`, run `pnpm install` to update
`pnpm-lock.yaml`, then run `dx generate --apply`. An import of a package listed
in the root `package.json` resolves to `//:node_modules/<package>`. Imports
that match `paths` in the nearest `tsconfig.json` resolve to local targets.

## Format, lint and typecheck

`biome` formats and lints TypeScript, and `tsc` checks types. Biome reads
`biome.json` at the workspace root. To lint with `eslint`, put
`eslint.config.mjs` at the root and bind it with `eslint_config`.

```sh
dx format --apply
dx lint
dx typecheck
```

## Platforms

See the [support matrix](https://github.com/devhusdk/rules_dx/blob/main/docs/support-matrix.md).
