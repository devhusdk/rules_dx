# GitHub CI

This page helps you run rules_dx checks, docs, updates and releases in GitHub Actions.

You call reusable workflows that rules_dx ships. A reusable workflow is a
workflow file in another repository that your own workflow runs with `uses:`.
You pin it to a rules_dx commit.

## Set up

1. Add rules_dx to your module. A module is your Bazel project, and
   `MODULE.bazel` at the repository root lists what it depends on. Add this
   line, as in [Adopting rules_dx in an existing Bazel repo](adopting-existing-bazel.md):

   ```python
   bazel_dep(name = "rules_dx", version = "...")
   ```

2. Write the workflow files:

   ```sh
   bazel run @rules_dx//:dx -- init --apply
   ```

   With `dx` installed, `dx init --apply` does the same. Init writes:

   - `.github/workflows/ci.yml`. When it already exists, init only updates the rules_dx commit in `uses:`.
   - `.github/workflows/dx-update.yml`, when missing.
   - `.github/workflows/release.yml`, when missing and a BUILD file calls `deployment` or a `*_deploy` macro from `@rules_dx//deploy:defs.bzl`.

   See [`dx init`](cli/commands/init.md).

3. Push the branch and open a pull request.

4. In branch protection, require the check `dx-ci (aggregate)`. Do not require the per-platform jobs.

5. Set the repository settings under [Repository settings](#repository-settings).

## Example: the CI workflow

A `ci.yml` that checks Linux x86_64 and dry-runs the release deployments looks like this:

<!-- snippet: examples/consumer-ci/caller.yml -->
```yaml
name: ci
on:
  pull_request: {}
  merge_group:
    types: [checks_requested]
  push:
    branches: [main]
  workflow_dispatch: {}
jobs:
  dx:
    uses: devhusdk/rules_dx/.github/workflows/reusable-consumer.yml@<commit>
    with:
      platforms: '["linux_x86_64"]'
      deploy_dry_run: true
    secrets:
      BUILDBUDDY_API_KEY: ${{ secrets.BUILDBUDDY_API_KEY }}
      REMOTE_CACHE_HEADER: ${{ secrets.REMOTE_CACHE_HEADER }}
    permissions:
      actions: read
      contents: write
      pull-requests: write
```

The `ci.yml` that init writes uses your default branch under `push`, pins `<commit>`, and sets `remote_cache: ${{ vars.DX_REMOTE_CACHE }}` in place of `platforms`. Add or change inputs under `with:`. The inputs are listed under [Inputs](#inputs).

When you open a pull request, you get one `checks (<platform>)` job per platform, a `security-audit` job per platform, and `dx-ci (aggregate)` with the overall result.

## What the job runs

A target is one thing Bazel can build or test, such as a library or a test. A label names a target, like `//pkg:lib`. The pattern `//...` means every target in the repository.

Most jobs first run `dx completion bash`. It writes the gitignored `.dx/bazelrc` files, so the next Bazel command reads the rules_dx Bazel settings.

Per platform, `checks (<platform>)` runs these in order. A failing check does not stop the next one.

| Check | Command | Reports |
|---|---|---|
| `lint` | `dx lint` | `lint.sarif` |
| `typecheck` | `dx typecheck` | `typecheck.sarif` |
| `format` | `dx format` | none |
| `generate` | `dx generate //...` | none |
| `license-audit` | `dx license //...` | none |
| `test` | `dx test --timings` | `junit.xml`, `failures.json`, `results.json` |
| `build` | `dx build --timings` | `failures.json` |
| `coverage` | `dx coverage` | `failures.json`, `results.json` |

The other jobs:

| Job | What it does |
|---|---|
| `validate (inputs)` | Checks the inputs. Runs first. |
| `advisory snapshots (one run, every platform)` | Prepares the advisory snapshots once for all platforms. |
| `security-audit` | Runs `dx security //...` in its own job per platform. |
| `advisory pin bump` | Runs when `advisory_pr` is `true`. Opens the advisory pin pull request. |
| `fuzz`, `bench`, `deploy dry run` | Run when their inputs are set. |
| `dx-ci (aggregate)` | Reports the result of `validate`, `checks`, `security-audit` and the advisory snapshots. |

Every check runs on every call. A skipped required job fails `dx-ci`. To turn a tool off, set it under `[families.<id>]` in `dx.toml` (see [Choose tools and tier](cli/commands/quality.md#choose-tools-and-tier)). CI and local runs then agree.

### Scope

On pull requests, `lint`, `typecheck`, `format`, `test`, `build` and `coverage` run with `--affected`, which picks only the targets your change reaches. Other events use `//...`.

- `coverage` uses `//...` when `min_coverage` or `min_changed_line_coverage` is set.
- Set `affected_on_pr: false` to use `//...` on pull requests.
- When the checkout is not a merge commit, scoped checks warn and run `//...`.
- When nothing is affected, the check passes and uploads no reports.

## Read a failure

1. Open the run summary. Each check has a line like `<check>: passed (exit 0), ran: tools/dx <check> ... <scope>` or `<check>: failed (exit N), ran: ...`. A scoped check with nothing affected adds `(nothing affected)`. A failed `lint` or `typecheck` also adds up to 10 annotations on the affected lines and a table of the first 50 findings.
2. Open the failed job log.
3. Download the reports with the GitHub CLI:

   ```sh
   gh run download <run-id> -n dx-reports-test-linux_x86_64
   gh run download <run-id> -p 'dx-reports-*'
   ```

Report artifacts:

- Names: `dx-reports-lint-<platform>`, `dx-reports-typecheck-<platform>`, `dx-reports-test-<platform>`, `dx-reports-build-<platform>` and `dx-reports-coverage-<platform>`. With `invocation_id` set, the name ends in `-<invocation_id>`.
- They are kept 3 days.
- They carry test output and log tails. Treat them like logs.
- The test and build artifacts also carry `profile-<check>.json`, the `--timings` report.

On a pull request from the same repository, each platform posts a summary comment. It shows coverage, cache hit rate, wall time and critical path. Pull requests from forks get the job summary only.

Common failures:

| Message | Fix |
|---|---|
| Rejected input | Use a value listed under [Inputs](#inputs). |
| `Config value 'ci' is not defined in any .rc file` | Add the `.bazelrc` lines under [Caches](#caches). |
| `advisory_refresh_failed` | Rerun the workflow, or run `bazel run @rules_dx//dx/advisory_prep --`. See [Advisory Snapshots](cli/commands/audit-update.md#advisory-snapshots). |
| Stale `MODULE.bazel.lock` | Run `bazel mod deps` and commit the lock file. |
| Skipped, cancelled or missing check | Rerun the pull request run. |

## Caches

A remote cache stores build and test results on a server, so later runs reuse them instead of redoing the work. A remote cache is recommended.

The workflows write a credential helper that reads the key from the environment and hands it to Bazel. You only pass the secret. Do not put the key in `--remote_header` or in any rc file.

1. For BuildBuddy, set the `BUILDBUDDY_API_KEY` secret:

   ```yaml
   secrets:
     BUILDBUDDY_API_KEY: ${{ secrets.BUILDBUDDY_API_KEY }}
   ```

2. For another REAPI cache, set `remote_cache`. If the cache needs a header, add the `REMOTE_CACHE_HEADER` secret as `name=value`. When it is set, `BUILDBUDDY_API_KEY` is not used.

   ```yaml
   with:
     remote_cache: grpcs://cache.example.com
   secrets:
     REMOTE_CACHE_HEADER: ${{ secrets.REMOTE_CACHE_HEADER }}
   ```

   The `ci.yml` that init writes reads `remote_cache` from the Actions variable `DX_REMOTE_CACHE`.

3. Add the cache URL to the `ci` config in `.bazelrc`. `.bazelrc` is the file of default Bazel flags at the repository root. A line that starts with `common:ci` applies only when Bazel runs with `--config=ci`, which the workflow passes when a cache is set.

   ```ini
   common:ci --remote_cache=grpcs://remote.buildbuddy.io
   common:ci --experimental_remote_downloader=grpcs://remote.buildbuddy.io
   common:ci --experimental_remote_downloader_local_fallback
   ```

`tools/dx init --apply` writes the rest of the `ci` and `ci-pr` configs to `.dx/bazelrc`:

- compression, timeouts, retries and local fallback
- all CPUs, `--jobs=64`, `--loading_phase_threads=32` and `--remote_max_connections=200`
- `--nobuild_runfile_links`
- an up-to-date `MODULE.bazel.lock`
- no uploads from pull requests

Runfiles are the files a program needs when it runs. Under `--config=ci`, `<bin>.runfiles` trees are not built. A script that builds and then runs `bazel-bin/...` and needs its runfiles breaks.

Without a remote cache, no config is passed.

For more, see [Remote cache](remote-cache.md).

The workflow keeps these GitHub Actions caches:

| Cache | When |
|---|---|
| Disk cache per `checks` job, keyed on the check and platform | Only without a remote cache. Saved on the default branch; other runs restore it. |
| Advisory snapshots under `.dx/local/advisory` | When `.dx/pins` pins them. |
| Fuzz corpus under `.dx/local/fuzz` | When `fuzz` is `true`. |

Bazel downloads and Bazelisk are fetched fresh in each job.

## Inputs

These are the `with:` inputs of `reusable-consumer.yml`.

| Input | Meaning |
|---|---|
| `platforms` | JSON array. Default `["linux_x86_64"]`. Values: `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64`. `[]` fails validation. |
| `min_coverage` | Whole number 0 to 100. Fails below it. Empty sets no threshold. |
| `min_changed_line_coverage` | Whole number 0 to 100. Pull requests only. Fails when fewer of the added executable lines are covered. Empty skips it. |
| `affected_on_pr` | Default `true`. Use `--affected` on pull requests. |
| `buf_breaking_on_pr` | Default `false`. On pull requests, run `lint` with `--breaking-against` so `buf breaking` compares the proto files with the pull request base. |
| `link_check_online` | Default `false`. Run `lint` with `--online` so lychee checks external links. Ignored on pull requests; call the workflow from a `schedule` trigger to use it. |
| `fuzz` | Default `false`. Run [`dx fuzz`](cli/commands/fuzz.md) on `dx-fuzz` targets on Linux x86_64. The corpus under `.dx/local/fuzz` is cached between runs. A crash fails the job and uploads the `dx-fuzz-crashes` artifact for 30 days; the log shows the reproduce line. |
| `fuzz_time` | Default `300s`. Time budget per fuzz target, such as `90s`, `5m` or `1h`. |
| `bench` | Default `false`. Run [`dx bench`](cli/commands/bench.md) on Linux x86_64. Pushes to the default branch upload `.dx/local/bench/last.json` as the `dx-bench-linux_x86_64` artifact for 90 days. Pull requests run `dx bench --compare <base branch> --max-regression=<bench_max_regression>` and add the table to the job summary. |
| `bench_max_regression` | Default `20`. Whole number 0 to 100. Percent a benchmark may be slower than the base branch before a pull request fails. |
| `deploy_dry_run` | Default `false`. Run [`dx deploy`](cli/commands/deploy.md), the dry run, on every `deployment` tagged `release` on Linux x86_64. The job runs `dx deploy --affected`. Pull requests with `affected_on_pr` dry-run only the deployments the change reaches. A Starlark or module change, or a deleted file, checks every release deployment. |
| `remote_cache` | REAPI cache URL. `dx init --apply` sets it to the `DX_REMOTE_CACHE` Actions variable. |
| `advisory_pr` | Default `false`. Run `dx update advisory --pr`, which opens or refreshes the `dx-update/advisory` pull request that moves the advisory pins in `.dx/pins` to the newest snapshot release. Call the workflow from a `schedule` trigger with `contents: write` and `pull-requests: write` permissions. |
| `invocation_id` | 1 to 40 letters, digits, underscores or hyphens. Set a different one per call when a run has several calls. |

| Secret | Meaning |
|---|---|
| `BUILDBUDDY_API_KEY` | BuildBuddy API key. Optional. |
| `REMOTE_CACHE_HEADER` | Header for `remote_cache` as `name=value`. Optional. |

Changed-line coverage runs `dx coverage --min-changed-line-coverage <percent>`. See [Build, test, and coverage](cli/commands/build-test-coverage.md) for output lines and error codes.

## Docs workflow

To check and publish an mdBook docs site:

1. Copy the YAML below to `.github/workflows/docs.yml`.
2. Set `site` to the label of the target that renders your site.
3. Before the first publish, set the Pages source to GitHub Actions in the repository settings.
4. In branch protection, require `docs-check`. Add `docs-publish (GitHub Pages)` when `publish` is true.

<!-- snippet: examples/docs-ci/caller.yml -->
```yaml
name: docs
on:
  pull_request: {}
  merge_group:
    types: [checks_requested]
  push:
    # Change to your default branch if it is not main.
    branches: [main]
  workflow_dispatch: {}
jobs:
  docs:
    uses: devhusdk/rules_dx/.github/workflows/reusable-docs.yml@d971da0e0d70907d291b84c44e3306efda22efe3
    with:
      docs_scope: "//..."
      site: "//docs:site"
      publish: ${{ github.event_name == 'push' && github.ref == format('refs/heads/{0}', github.event.repository.default_branch) }}
    secrets:
      BUILDBUDDY_API_KEY: ${{ secrets.BUILDBUDDY_API_KEY }}
    permissions:
      contents: read
      pages: write
      id-token: write
```

`docs-publish (GitHub Pages)` does not run on pull requests.

| Input | Meaning |
|---|---|
| `docs_scope` | Space-separated target patterns for `dx lint`. Default `//...`. |
| `site` | Required. Target label whose output is the rendered mdBook tree. |
| `site_check` | Target label for `dx docs`. Default empty, which skips it. |
| `publish` | Deploy to Pages. Default `false`. Needs `pages: write` and `id-token: write`. |
| `environment` | Pages environment. Default `github-pages`. |
| `forbid` | Text the site must not contain, one per line. |
| `landing` | Require a landing page. Default `false`. |
| `smoke_test` | Test label run with `DX_PAGES_URL` set, after the deploy. |

## Dependency updates

`dx-update.yml` runs `tools/dx update --pr` every Monday at 04:00 UTC and on `workflow_dispatch`. To change it, edit `cron` and the flags in the file.

1. Allow auto-merge in the repository settings.
2. Allow GitHub Actions to create and approve pull requests (Settings, Actions, General, Workflow permissions).
3. For CI on update pull requests and for Actions pins, create a GitHub App with read and write on Contents, Pull requests and Workflows.
4. Set the secrets `DX_UPDATE_APP_ID` and `DX_UPDATE_APP_PRIVATE_KEY`.

Without the App secrets, the job warns and CI does not run on the update pull requests.

## Releases

`release.yml` calls `reusable-release.yml` when you push a `v*` tag.

1. Tag each `deployment` target to publish with `tags = ["release"]`. The tag must be exactly `release`; `prerelease` and `release-notes` do not match.
   A `github_deploy` also needs `tag_from_head = True`. Without it the release tag is `v0.0.0-dryrun`: the dry run passes and the publish fails with `github_deploy: live publish refuses placeholder version v0.0.0-dryrun; set a real version`.
2. Store registry tokens, such as `NPM_API_TOKEN`, as repository secrets. The caller passes `secrets: inherit`.
3. Push a tag:

   ```sh
   git tag v1.0.0
   git push origin v1.0.0
   ```

What runs:

1. `plan` lists the deployables.
2. `dry run (<platform>)` runs `dx deploy <label>` for each one on every platform in `platforms`, so the dry run is in the log.
3. `release` runs `dx deploy --apply <label>` for each, in the order of the list, with only the `*_APPROVED` variables set. A failed deployable stops the rest. The job summary lists the ones already published and a `gh workflow run <caller>.yml` command, naming the workflow that called `reusable-release.yml`, that resumes with the rest.

A `github_deploy` creates a draft release. With signing, each asset has a `<asset>.sigstore.json`. Verify an asset with either command:

```sh
gh attestation verify <asset> -R <owner>/<repo> \
  --signer-workflow devhusdk/rules_dx/.github/workflows/reusable-release.yml
cosign verify-blob <asset> --bundle <asset>.sigstore.json \
  --certificate-identity-regexp '^https://github\.com/devhusdk/rules_dx/\.github/workflows/reusable-release\.yml@' \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
```

`publish` gates the `release` job, which also needs a tag ref. With `publish: false`, the plan and dry run run on any ref and nothing is published. The `release.yml` that `dx init` writes has a `workflow_dispatch` with `publish` defaulting to `false`, so dispatching it from a branch only plans and dry-runs. Pass `true` on a tag to publish. Tag pushes always publish.

An existing `release.yml` keeps its triggers. Add the `workflow_dispatch` `publish` input by hand, and this line under `with:`:

```yaml
publish: ${{ github.event_name != 'workflow_dispatch' || inputs.publish }}
```

The caller grants `contents`, `id-token`, `attestations` and `packages` write.

A `pypi_deploy` through this workflow uploads with `PYPI_API_TOKEN` and without attestations. For trusted publishing and attestations, run the upload step in a workflow file of your own repository. See [`pypi_deploy`](https://github.com/devhusdk/rules_dx/blob/main/deploy/README.md#pypi_deploy).

| Input | Meaning |
|---|---|
| `deployables` | JSON array of labels, for example `["//pkg:npm", "//pkg:github"]`. Empty publishes every `deployment` tagged `release`. |
| `platforms` | JSON array of platforms for the dry run. Default `["linux_x86_64"]`. |
| `publish` | Default `true`. Publish after the dry run. |
| `runner` | Runner label for the `release` job. Default `ubuntu-latest`. |

## Repository settings

- Approval policy for fork pull request workflows: `all_external_contributors`.
- Merge gate: require up-to-date branches, or use a merge queue.
- Workflow permissions: read-only default.
