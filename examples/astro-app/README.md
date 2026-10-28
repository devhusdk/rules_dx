# Astro app example

A standalone Astro consumer. It has its own `package.json` and `pnpm-lock.yaml`, so the
pinned `astro` version comes from this directory and not from the repository root.

```sh
bazel build //examples/astro-app:site
bazel test //examples/astro-app:check
```
