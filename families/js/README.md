# JavaScript and TypeScript

JavaScript and TypeScript tests and fixtures. They live in the `rules_dx_js` module, a
`bazel_dep` next to `rules_dx`. The rules, the toolchains and the quality tools stay
in `rules_dx`, because its own tool targets use them.

- [JavaScript](javascript/README.md)
- [TypeScript](typescript/README.md)

The module links its test packages from its own `package.json` and `pnpm-lock.yaml` into
`//:node_modules`, through the `@dx_js_npm` hub. The `node-app` fixture keeps its own lock in
`@npm_node_app`.
