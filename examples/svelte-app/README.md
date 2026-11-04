# Svelte app example

A standalone Svelte 5 app with its own `package.json`, `pnpm-lock.yaml` and `tsconfig.json`. It is built for a consumer module that depends on `rules_dx` under another repository name.

`consumer.BUILD.bazel` is the `BUILD.bazel` that consumer uses. It loads `svelte_library`, `svelte_build` (with `ssr_entry`) and `svelte_check` from `@rules_dx`, and runs one rendered SSR test and one client interaction test.
