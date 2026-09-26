# GitHub CI

Run `dx` in GitHub Actions with the reusable workflows. Copy the starters into
your repo and pin a reviewed commit.

```sh
cp examples/consumer-ci/caller.yml .github/workflows/ci.yml
cp examples/docs-ci/caller.yml .github/workflows/docs.yml
```

```python
bazel_dep(name = "rules_dx", version = "0.0.0")
```

## Consumer Workflow

`examples/consumer-ci/` calls `reusable-consumer.yml`. Each check runs
`dx <check> --check` on your platforms: `lint`, `typecheck`, `format`,
`generate`, `security`, `license`, `test`, `build`, `coverage`.

Inputs:

- `rules_dx_version`: must match your `MODULE.bazel` pin.
- `platforms`: defaults to Linux x86_64. Add arm64, macOS, Windows as needed.
- `min_coverage`: fails below that percent. Coverage comes from `dx coverage`.

The workflow needs `contents: read` plus `checks: write` for check runs.
Pull requests use the read-only cache config automatically.

## Docs Workflow

`examples/docs-ci/` calls `reusable-docs.yml`. It runs `dx lint --check` over
`docs_scope` (default `//...`) plus `dx docs --check` site validation, so
broken links fail before deploy.

Inputs:

- `rules_dx_version`: must match your `MODULE.bazel` pin.
- `docs_scope`: scope covered by the docs check. Default `//...`.
- `publish`: build the rendered site and deploy to Pages. Default false.
- `environment`: Pages environment name. Default `github-pages`.

Publishing needs `pages: write` plus `id-token: write`. Enable Pages with
source GitHub Actions before the first publishing run.

## Caching

Pass `BUILDBUDDY_API_KEY` to share the BuildBuddy remote cache. Fork pull
requests run local-cache only. Codecov is optional.
