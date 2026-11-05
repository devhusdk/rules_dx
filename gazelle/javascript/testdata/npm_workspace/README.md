# npm links in a pnpm workspace

Verifies `npm_link_all_packages` is generated at the directory holding
`pnpm-lock.yaml` and in each workspace package it lists, and nowhere else.

The load names `@npm`, or the hub that `npm_translate_lock` declares in
`MODULE.bazel`. `# gazelle:map_kind npm_link_all_packages npm_link_all_packages @<hub>//:defs.bzl`
overrides it.
