# Node app example

A multi-package TypeScript and JavaScript app. It has a package registry CLI, a `registry` library that checks versions through `semver`, a generated `build_info` module and a packaged `packages.json` resource.

| Package | Role |
| --- | --- |
| `registry` | Typed registry model, `semver` helpers, generated `build_info.ts`, `packages.json` |
| `cli` | `registry` binary with `list`, `latest`, `next`, `satisfy`, `schema` and `where` |
| `diagnostics` | Files with deliberate errors, and the checks that must reject them |

```sh
bazel build //examples/node-app/...
bazel test //examples/node-app/...
bazel run //examples/node-app/cli:registry -- list
bazel run //examples/node-app/cli:registry -- next core minor
bazel run //examples/node-app/cli:registry -- satisfy net '^2.0.0'
bazel run //examples/node-app/cli:registry -- --file path/to/packages.json list
```

Without `--file` or `REGISTRY_FILE`, `registry` reads the packaged `packages.json`, so the binary runs from any directory. Relative `--file` paths resolve against the directory you ran `bazel run` from. `latest` and `next` ignore prerelease versions.

Exit codes: `0` ok, `2` usage or invalid range, `3` unreadable or malformed registry file, `4` unknown package or no matching version. `exit_codes_test` runs the binary through each code. `registry where` prints the entry script, the data file and the `semver` directory the binary resolved.

Tests use both runners. `registry_test` and `cli_test` run Jest. `plain_test` runs `node:test`. Every TypeScript target also gets a typecheck test with `isolatedDeclarations`. `semver` and the test tools come from the locked `npm_node_app` hub. Refresh the lock with `pnpm install --lockfile-only` in a copy of `package.json` and `pnpm-workspace.yaml`.

Sources use the repo Biome settings. `format_test` checks every source and config file against the pinned Biome. `diagnostics_test` runs `tsc` and Biome on files with a syntax error, a type error, an inferred export under `isolatedDeclarations`, an import of a package that is not installed, a lint finding in `.ts` and `.js`, and bad formatting, and expects each to fail.

`bazel coverage //examples/node-app/...` writes LCOV for the Jest tests: `registry.js`, `build_info.js` and `cli.js`. `coverage_test` checks each report names those files with executed lines. `plain_test` runs `node:test` and reports none.
