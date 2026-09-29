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

`examples/consumer-ci/` calls `reusable-consumer.yml`. Nine checks run on
every platform in `platforms`:

- `lint`: `dx lint --check //...`
- `typecheck`: `dx typecheck --check //...`
- `format`: `dx format --check //...`
- `generate`: `dx generate --check //...`
- `security-audit`: `dx security //...`
- `license-audit`: `dx license //...`
- `test`: `dx test //...`
- `build`: `dx build //...`
- `coverage`: `dx coverage //...`, plus `--min-coverage <percent>` when
  `min_coverage` is set.

`--check` applies to the four quality checks. `dx security`, `dx license`,
`dx test`, `dx build`, and `dx coverage` reject it.

Inputs:

- `rules_dx_version`: must match your `MODULE.bazel` pin.
- `disabled_checks`: job IDs above to skip. Empty runs all nine.
- `platforms`: JSON array of platform labels, such as `["linux_x86_64"]`.
  Defaults to `["linux_x86_64"]`. Valid labels: `linux_x86_64`, `linux_arm64`,
  `macos_arm64`, `windows_x86_64`, `windows_arm64`. Any other label fails the
  workflow.
- `scheduling_mode`: `parallel` or `sequential`. Checks run in parallel either way.
- `code_scanning_opt_in`: accepted and ignored.
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

## Repository Settings

Neither caller configures these. Set them once in the repository.

- Approval policy: `all_external_contributors`, so fork pull requests run.
- Branch protection for the consumer workflow: require `dx-ci (aggregate)`.
  One green gate covers the whole matrix.
- Branch protection for the docs workflow: require `docs-check`. Add
  `docs-publish (GitHub Pages)` once `publish` is true.
- Merge gate: require branches to be up to date, or use a merge queue.
- Pages source: GitHub Actions, before the first publishing run.
- Workflow permissions: read-only by default. The nine check jobs need
  `checks: write`; `coverage` and the `dx-ci` aggregate also need
  `pull-requests: write`.

## Caching

Both callers forward repository secrets with `secrets: inherit`. Set
`BUILDBUDDY_API_KEY` to share the BuildBuddy remote cache. Fork pull
requests run local-cache only. Codecov is optional.
