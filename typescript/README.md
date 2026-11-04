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
`entry_point` to run the file with Node instead.
On Windows, Jest coverage uses the v8 provider.

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
