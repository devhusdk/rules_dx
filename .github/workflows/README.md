# CI Workflows

- `reusable-consumer.yml`: nine checks plus the `dx-ci (aggregate)` gate.
- `reusable-docs.yml`: `docs-check` plus `docs-publish` (GitHub Pages).
- `ci.yml`: this repository's own `ci` gate.
- `bump.yml`: `widen-one-requirement loop` for one dependency per run.
- `ghcr.yml`: prebuilt devcontainer images.
- `publish-dry-run.yml`: the release path without a release.

The consumer contract is [GitHub CI](../../docs/github-ci.md).
