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

## Pin Check

Set `rules_dx_version` to the version of your `rules_dx` `bazel_dep`. The
`validate (inputs)` job compares the two before any check starts. A repository
that calls the workflow from its own tree declares itself instead:

```python
module(
    name = "rules_dx",
    version = "0.0.0",
)
```

A mismatch fails `validate`, so no check runs:

```text
MODULE.bazel:<line> declares rules_dx "<x>", the workflow expects "<y>". Pin both to the same version.
```

The check reads `MODULE.bazel` and every file it pulls in with
`include("//pkg:name.MODULE.bazel")`, nested includes too. `rules_dx` may be
declared once across all of them, and diagnostics name the file and line. A
missing, repeated, non-literal or non-`//` include fails.

It also fails when `MODULE.bazel` declares no `rules_dx`, declares it twice, or
gives a version that is not a quoted string. A `rules_dx` release too old for
the check fails in Bazel before the check runs.

## Consumer Workflow

`examples/consumer-ci/` calls `reusable-consumer.yml`. Nine checks run on
every platform in `platforms`:

- `lint`: `dx lint --check //...` (`--affected` on pull requests), with a SARIF report
- `typecheck`: `dx typecheck --check //...` (`--affected` on pull requests), with a SARIF report
- `format`: `dx format --check //...` (`--affected` on pull requests)
- `generate`: `dx generate --check //...`
- `security-audit`: `dx security //...`
- `license-audit`: `dx license //...`
- `test`: `dx test //...` (`--affected` on pull requests), with JUnit, failures and results reports
- `build`: `dx build //...` (`--affected` on pull requests), with a failures report
- `coverage`: `dx coverage //...` (`--affected` on pull requests when no threshold is set) with failures and results reports, plus `--min-coverage <percent>` when
  `min_coverage` is set. With `min_changed_line_coverage` on pull requests it
  adds `--min-changed-line-coverage <percent>`.

Each check reports once per platform as `<check> (<platform>)`, for example
`lint (linux_x86_64)`.

`--check` applies to the four quality checks. `dx security`, `dx license`,
`dx test`, `dx build`, and `dx coverage` reject it.

`validate (inputs)` runs first and checks the pin and every input. One
`advisory snapshots (one run, every platform)` job fetches the advisory
data once per run for every `security-audit` cell. `dx-ci (aggregate)` passes
only when every check that should run succeeds.

Inputs:

- `rules_dx_version`: the version your `rules_dx` declaration carries. The
  pin check compares it with `MODULE.bazel`.
- `disabled_checks`: job IDs above to skip, comma-separated. Empty runs all
  nine. Surrounding spaces are ignored. An unknown ID, a repeated ID and an
  empty ID fail.
- `platforms`: JSON array of platform labels, such as `["linux_x86_64"]`. The label-to-runner table is `PLATFORMS` in `tools/ci/input_validation.py`; every other copy is pinned to it by `input_validation_test`.
  Omit it to run `["linux_x86_64"]`. Valid labels: `linux_x86_64`, `linux_arm64`,
  `macos_arm64`, `windows_x86_64`, `windows_arm64`. Malformed JSON, a JSON value
  that is not an array, an empty array, a repeated label, an unknown label and a
  label carrying spaces all fail.
- `min_coverage`: fails below that percent, a whole number from 0 to 100 such as
  `80`. Empty collects without enforcing a threshold. `80%`, `80.5`, `-1`, `101`,
  `eighty` and a leading space fail. Coverage comes from `dx coverage`.
- `min_changed_line_coverage`: fails when fewer than that percent of the
  executable lines a pull request adds are covered, a whole number from 0 to
  100. Empty skips the check. It accepts and rejects the same values as
  `min_coverage`, and it never changes `min_coverage`.
- `affected_on_pr`: boolean, default `true`. On `pull_request` events, `lint`,
  `typecheck`, `format`, `test` and `build` run with `--affected`. Set `false`
  to keep `//...` on pull requests. See "Affected Scope on Pull Requests".
- `invocation_id`: give each call in one workflow run its own ID, 1 to 40
  letters, digits, underscores or hyphens, such as `api-linux`. Empty suits
  one call per run. The ID suffixes the report and `advisory-snapshots`
  artifacts, as in `dx-reports-test-linux_x86_64-api-linux`.

Grant `contents: read` and `pull-requests: write` on the calling job;
`coverage` posts its pull-request comment with it.
`examples/consumer-ci/caller.yml` does.

## Affected Scope on Pull Requests

On `pull_request` events the scoped checks run only the targets a pull request
affects. Each scoped job fetches two commits and points the base ref at the first
parent of the pull request merge commit, so the base is the tip of the branch
the pull request targets. `push` and `merge_group` events keep `//...`.

- `coverage` falls back to `//...` when `min_coverage` or
  `min_changed_line_coverage` is set, because those gates need whole-workspace
  results.
- `generate`, `security-audit` and `license-audit` always use `//...`.
- An edit to `MODULE.bazel` or another workspace-wide input selects every target.
- When nothing is affected, the job passes and uploads no report files.

## Report Artifacts

Each report-capable job uploads its machine-readable reports with
`if: always()`, so a red run still leaves them. One artifact per job and
platform is named `dx-reports-<job>-<platform>`, plus `-<invocation_id>` when
that input is set. Artifacts are kept for 14 days. A missing report only warns.

| Job | Artifact | Files |
|---|---|---|
| `lint` | `dx-reports-lint-<platform>` | `lint.sarif` |
| `typecheck` | `dx-reports-typecheck-<platform>` | `typecheck.sarif` |
| `test` | `dx-reports-test-<platform>` | `junit.xml`, `failures.json`, `results.json` |
| `build` | `dx-reports-build-<platform>` | `failures.json` |
| `coverage` | `dx-reports-coverage-<platform>` | `failures.json`, `results.json` |

`format`, `generate`, `security-audit` and `license-audit` upload no reports;
`dx` has no report kind for the first two. Download one artifact or all of them:

```sh
gh run download <run-id> -n dx-reports-test-linux_x86_64
gh run download <run-id> -p 'dx-reports-*'
```

Artifacts are not masked like logs. The results and JUnit reports carry test
output and log tails, so treat them as you treat the logs.

## Changed-Line Coverage

Set `min_changed_line_coverage` to require coverage on new code without raising
the whole-repo threshold:

```yaml
    with:
      rules_dx_version: "0.0.0"
      min_changed_line_coverage: "80"
```

It runs on `pull_request` events only, as
`dx coverage --min-changed-line-coverage <percent>` in the `coverage` job. Other events
print a notice and skip it. The job fetches full history and points
`origin/HEAD` at the first parent of the pull request merge commit, or at the
pull request base commit when the checkout is not a merge commit. Deleted lines
and lines the coverage report does not list, such as comments and docs, do not
count. Lines under `LCOV_EXCL_*` markers do not count either. With no line to
count the result is `not-applicable`, not 100 percent.

The `dx: changed…` lines from the coverage log go to the step summary: the
covered and eligible line counts and each uncovered `path:line`. The
`coverage` job fails when:

- fewer than the threshold percent of eligible lines are covered. The compare is
  exact, with no rounding.
- the base commit is not available.
- a changed production source that Bazel instruments has no record in the
  report. `dx coverage --report sources=<path>` lists these sources. Test
  sources and files no instrumented target owns are skipped. The result is
  `incomplete`. A library only reached by `no-coverage` tests stays
  `incomplete`.

See `dx coverage` in the [command reference](cli/commands/build-test-coverage.md)
for the output lines and error codes.

## Docs Workflow

`examples/docs-ci/` calls `reusable-docs.yml`. It runs `dx lint --check` over
`docs_scope` (default `//...`), runs `dx docs --check` on `site_check` when
you set it, then renders your `site` label with `dx docs` and checks the
rendered tree for missing pages, styles, and search index, so broken links
fail before deploy.

Inputs:

- `rules_dx_version`: the version your `rules_dx` declaration carries. The
  pin check compares it with `MODULE.bazel`.
- `docs_scope`: target patterns covered by the docs check, separated by
  spaces, such as `//... //docs:all`. Default `//...`. Each pattern reaches
  `dx` as one argument. A value with quotes, shell characters, wildcards, a
  leading `-` or anything but a target pattern fails with one error naming it.
- `publish`: build the rendered site and deploy to Pages. Default false.
- `environment`: Pages environment name. Default `github-pages`.
- `site`: required. One target label in your repository whose output is the
  rendered mdBook tree, with `index.html` and `searchindex.js` at its root.
- `site_check`: target label that `dx docs --check` runs. Default empty,
  which skips the source check.
- `forbid`: text the rendered site must not contain, one entry per line.
  Default empty.
- `landing`: also require a landing page in the rendered site. Default false.
- `smoke_test`: test label run with `DX_PAGES_URL` set to the deployed page
  URL. Default empty, which runs only the built-in page fetch.

A `site`, `site_check` or `smoke_test` value that is not a single target label
fails the docs check with one `dx-ci:` line naming the input.

Publishing needs `pages: write` plus `id-token: write`. The publish job stages
and validates the whole `site` tree, deploys it, fetches `index.html` and
`searchindex.js` from the reported page URL, then runs `smoke_test` if you set
one, all before the clean-checkout proof. Enable Pages with source GitHub
Actions before the first publishing run.

## Repository Settings

Set these once in the repository. Each caller already grants its job
permissions, so a read-only repository default is fine.

- Approval policy: `all_external_contributors`, so fork pull requests run.
- Branch protection for the consumer workflow: require `dx-ci (aggregate)`.
  One green gate covers the whole matrix. GitHub counts a skipped required
  check as passing, so rerun a cancelled pull request run before merging, or
  use a merge queue.
- Branch protection for the docs workflow: require `docs-check`. Add
  `docs-publish (GitHub Pages)` once `publish` is true.
- Merge gate: require branches to be up to date, or use a merge queue. Both
  callers run on `merge_group` (`checks_requested`), so the queue gets the
  required checks for its own SHA. Merge-group runs post no PR comment and
  never publish Pages.
- Pages source: GitHub Actions, before the first publishing run.
- Workflow permissions: read-only by default. Each calling job then grants
  what its reusable jobs request: the consumer caller `pull-requests: write`, the docs caller `pages: write` plus
  `id-token: write`.

## Caching

Both callers forward repository secrets with `secrets: inherit`. Set
`BUILDBUDDY_API_KEY` to share the BuildBuddy remote cache. Codecov is
optional.

Every job restores the Bazel repository cache that Bazel and `dx` use. Each
platform shares one copy, keyed on `MODULE.bazel` and `MODULE.bazel.lock`.
On the default branch, `coverage` saves it. If `coverage` is disabled, `test`
saves it. If both are disabled, `build` saves it. With all three disabled,
nothing saves it. The saved copy holds only the downloads the saving job
used. Pull requests, forks included, merge queues and other branches only
restore it. Without `BUILDBUDDY_API_KEY`, build and test outputs
are not cached between runs.

With that secret set, every Bazel and `dx` call in both workflows runs with
`--config=ci`, plus `--config=ci-pr` on pull requests. Define both configs in
the repository `.bazelrc` before setting the secret. An undefined config fails
the run: `Config value 'ci' is not defined in any .rc file`.

```ini
common:ci --remote_cache=grpcs://remote.buildbuddy.io
common:ci --remote_cache_compression
common:ci --remote_timeout=10m
common:ci --remote_download_outputs=all
common:ci-pr --noremote_upload_local_results
```

Without the secret, no config flag is passed and no stanza is needed.

## Failures

- Pin mismatch: `validate (inputs)` fails and names both versions. Set
  `rules_dx_version` to your `bazel_dep` version.
- Rejected input: `validate (inputs)` fails with the accepted values and no
  check runs. `dx-ci (aggregate)` fails with it.
- `Config value 'ci' is not defined in any .rc file`: add the `.bazelrc`
  stanzas under [Caching](#caching).
- `advisory_refresh_failed`: the advisory data was missing or stale. Rerun the
  workflow, or refresh it locally with `bazel run @rules_dx//cli/advisory_prep --`.
  See [Advisory Snapshots](cli/commands/audit-update.md#advisory-snapshots).
- A check skipped without being listed in `disabled_checks`, a cancelled check
  or a check with no result: `dx-ci (aggregate)` fails. Rerun a cancelled
  pull request run before merging.
