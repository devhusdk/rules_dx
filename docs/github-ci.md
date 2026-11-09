# GitHub CI

## Set up

1. Add the `bazel_dep` and write the workflows:

   ```sh
   bazel run @rules_dx//:dx -- init --apply
   ```

   This writes `.github/workflows/ci.yml`, `.github/workflows/dx-update.yml` when missing, and `.github/workflows/release.yml` once a BUILD file declares a `dx_deployment`. See [Init](cli/commands/init.md).

2. Check `ci.yml`:

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
       secrets:
         BUILDBUDDY_API_KEY: ${{ secrets.BUILDBUDDY_API_KEY }}
         REMOTE_CACHE_HEADER: ${{ secrets.REMOTE_CACHE_HEADER }}
       permissions:
         actions: read
         contents: write
         pull-requests: write
   ```

3. Push the branch and open a pull request.

4. Require `dx-ci (aggregate)` in branch protection. Do not require the per-platform jobs.

5. For docs, copy the docs starter and set `site`:

   ```sh
   cp examples/docs-ci/caller.yml .github/workflows/docs.yml
   ```

   Require `docs-check`. Add `docs-publish (GitHub Pages)` when `publish` is true.

## What the job runs

Every job first runs `dx completion bash` to write the gitignored `.dx/bazelrc` layers, so the next Bazel call reads them.

Per platform, `checks (<platform>)` runs these in order. A failing check does not stop the next one.

| Check | Command | Reports |
|---|---|---|
| `lint` | `dx lint` | `lint.sarif` |
| `typecheck` | `dx typecheck` | `typecheck.sarif` |
| `format` | `dx format` | none |
| `generate` | `dx generate //...` | none |
| `license-audit` | `dx license //...` | none |
| `test` | `dx test` | `junit.xml`, `failures.json`, `results.json` |
| `build` | `dx build` | `failures.json` |
| `coverage` | `dx coverage` | `failures.json`, `results.json` |

`security-audit` runs `dx security //...` in its own job per platform. `validate (inputs)` checks the inputs first. `advisory snapshots (one run, every platform)` prepares the advisory snapshots once for all platforms. With `advisory_pr`, `advisory pin bump` opens the advisory pin pull request. `fuzz` and `bench` run when their inputs are set. `dx-ci (aggregate)` reports the result of the required jobs. Every check runs on every call; a skipped required job fails `dx-ci`. To turn a tool off, set it in the quality policy's `families` (see [quality](cli/commands/quality.md)), so CI and local runs agree.

On pull requests, `lint`, `typecheck`, `format`, `test`, `build` and `coverage` run with `--affected`. Other events use `//...`. `coverage` uses `//...` when `min_coverage` or `min_changed_line_coverage` is set. Set `affected_on_pr: false` to use `//...` on pull requests.

When the checkout is not a merge commit, scoped jobs warn and run `//...`. When nothing is affected, the job passes and uploads no reports.

## Read a failure

1. Open the run summary. It lists `<check>: passed (exit 0), ran: tools/dx <check> ... <scope>` or `failed (exit N), ran: ...` for each check. A scoped check with nothing affected adds `(nothing affected)`. A failed `lint` or `typecheck` also adds up to 10 annotations on the affected lines and a table of the first 50 findings.
2. Open the failed job log.
3. Download the reports:

   ```sh
   gh run download <run-id> -n dx-reports-test-linux_x86_64
   gh run download <run-id> -p 'dx-reports-*'
   ```

   Artifacts are named `dx-reports-lint-<platform>`, `dx-reports-typecheck-<platform>`, `dx-reports-test-<platform>`, `dx-reports-build-<platform>` and `dx-reports-coverage-<platform>`, plus `-<invocation_id>` when set. They are kept 3 days. Reports carry test output and log tails. Treat them like logs.

   The test and build artifacts also carry `profile-<check>.json`, the `dx --timings` report. The PR summary comment shows its cache hit rate, wall time and critical path per platform.

Common failures:

| Message | Fix |
|---|---|
| Rejected input | Use a value listed under Inputs. |
| `Config value 'ci' is not defined in any .rc file` | Add the `.bazelrc` stanzas under Caches. |
| `advisory_refresh_failed` | Rerun the workflow, or run `bazel run @rules_dx//cli/advisory_prep --`. See [Advisory Snapshots](cli/commands/audit-update.md#advisory-snapshots). |
| Stale `MODULE.bazel.lock` | Run `bazel mod deps` and commit the lock. |
| Skipped, cancelled or missing check | Rerun the pull request run. |

## Caches

The reusable workflows write a credential helper that reads the key from the environment and pass it to Bazel. Callers only pass the secret. Do not put the key in `--remote_header` or any rc file.

1. For BuildBuddy, set the `BUILDBUDDY_API_KEY` secret:

   ```yaml
   secrets:
     BUILDBUDDY_API_KEY: ${{ secrets.BUILDBUDDY_API_KEY }}
   ```

2. For another REAPI cache, set `remote_cache`. Add the `REMOTE_CACHE_HEADER` secret (`name=value`) if it needs a header:

   ```yaml
   with:
     remote_cache: grpcs://cache.example.com
   secrets:
     REMOTE_CACHE_HEADER: ${{ secrets.REMOTE_CACHE_HEADER }}
   ```

3. With a remote cache, add its URL to the `ci` config in `.bazelrc`:

   ```ini
   common:ci --remote_cache=grpcs://remote.buildbuddy.io
   common:ci --experimental_remote_downloader=grpcs://remote.buildbuddy.io
   common:ci --experimental_remote_downloader_local_fallback
   ```

   `tools/dx init --apply` writes the rest of `ci` and `ci-pr` to `.dx/bazelrc`: compression, timeouts, retries, local fallback, all CPUs, `--jobs=64`, `--loading_phase_threads=32`, `--remote_max_connections=200`, `--nobuild_runfile_links`, an up-to-date `MODULE.bazel.lock`, and no uploads from pull requests. Under `--config=ci`, `<bin>.runfiles` trees are no longer built, so a script that builds and then runs `bazel-bin/...` relying on runfiles breaks.

   Without a remote cache, no config is passed. The `.dx/bazelrc` preset sets `startup --experimental_remote_repo_contents_cache`, which stores fetched repositories in the remote cache when one is set and does nothing otherwise. It applies to local runs too. With 32 loading threads, the lookups run in parallel.

A remote cache is recommended.

The workflow keeps these GitHub Actions caches:

| Cache | When |
|---|---|
| Disk cache per `checks` cell, keyed on the check and platform | Only without a remote cache. Saved on the default branch; other runs restore it. |
| Advisory snapshots under `.dx/local/advisory` | When `.dx/pins` pins them. |
| Fuzz corpus under `.dx/local/fuzz` | When `fuzz` is `true`. |

Bazel downloads and Bazelisk are fetched fresh in each job.

## Inputs

| Input | Meaning |
|---|---|
| `platforms` | JSON array. Default `["linux_x86_64"]`. Values: `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64`. |
| `min_coverage` | Whole number 0 to 100. Fails below it. Empty sets no threshold. |
| `min_changed_line_coverage` | Whole number 0 to 100. Pull requests only. Fails when fewer of the added executable lines are covered. Empty skips it. |
| `affected_on_pr` | Default `true`. Use `--affected` on pull requests. |
| `buf_breaking_on_pr` | Default `false`. On pull requests, run `lint` with `--breaking-against` so `buf breaking` compares the proto files with the pull request base. |
| `link_check_online` | Default `false`. Run `lint` with `--online` so lychee checks external links. Ignored on pull requests; call the workflow from a `schedule` trigger to use it. |
| `fuzz` | Default `false`. Run [`dx fuzz`](cli/commands/fuzz.md) on `dx-fuzz` targets on Linux x86_64. The corpus under `.dx/local/fuzz` is cached between runs. A crash fails the job and uploads the `dx-fuzz-crashes` artifact for 30 days; the log shows the reproduce line. |
| `fuzz_time` | Default `300s`. Time budget per fuzz target: `90s`, `5m` or `1h`. |
| `bench` | Default `false`. Run [`dx bench`](cli/commands/bench.md) on Linux x86_64. Pushes to the default branch upload `.dx/local/bench/last.json` as the `dx-bench-linux_x86_64` artifact for 90 days. Pull requests run `dx bench --compare <base branch> --max-regression=<bench_max_regression>` and add the table to the job summary. |
| `bench_max_regression` | Default `20`. Percent a benchmark may be slower than the base branch before a pull request fails. |
| `remote_cache` | REAPI cache URL. `dx init --apply` wires it to the `DX_REMOTE_CACHE` repository variable. |
| `advisory_pr` | Default `false`. Run `dx update advisory --pr`, which opens or refreshes the `dx-update/advisory` pull request that moves the advisory pins in `.dx/pins` to the newest snapshot release. Call the workflow from a `schedule` trigger with `contents: write` and `pull-requests: write` permissions. |
| `invocation_id` | 1 to 40 letters, digits, underscores or hyphens. Set one per call when a run has several calls. |

The `disabled_checks` input is gone. Callers must delete `disabled_checks` from their `with:` block, or the call fails with an undefined-input error.

Changed-line coverage runs `dx coverage --min-changed-line-coverage <percent>`. See [Build, test, coverage](cli/commands/build-test-coverage.md) for output lines and error codes.

## Docs workflow inputs

| Input | Meaning |
|---|---|
| `docs_scope` | Space-separated target patterns for `dx lint`. Default `//...`. |
| `site` | Required. Target label whose output is the rendered mdBook tree. |
| `site_check` | Target label for `dx docs`. Default empty. |
| `publish` | Deploy to Pages. Default `false`. Needs `pages: write` and `id-token: write`. |
| `environment` | Pages environment. Default `github-pages`. |
| `forbid` | Text the site must not contain, one per line. |
| `landing` | Require a landing page. Default `false`. |
| `smoke_test` | Test label run with `DX_PAGES_URL` set. |

Set the Pages source to GitHub Actions before the first publish.

## Dependency updates

`dx-update.yml` runs `tools/dx update --pr` every Monday at 04:00 UTC and on `workflow_dispatch`. Edit `cron` and the flags to change it.

1. Allow auto-merge in the repository settings.
2. Allow GitHub Actions to create and approve pull requests.
3. For CI on update pull requests and for Actions pins, create a GitHub App with read and write on Contents, Pull requests and Workflows.
4. Set the secrets `DX_UPDATE_APP_ID` and `DX_UPDATE_APP_PRIVATE_KEY`.

## Releases

`release.yml` calls `reusable-release.yml` when a `v*` tag is pushed. Tag the `dx_deployment` targets to publish with `tags = ["release"]`; the tag must be exactly `release`, so `prerelease` and `release-notes` do not match. A dry-run job runs `dx deploy <label>` for each one on every platform in the `platforms` input (default `linux_x86_64`) so the dry run is in the log, then the release job runs `dx deploy --apply <label>` for each, in the order of the list, with only the `*_APPROVED` variables set. A failed deployable stops the rest, and the job summary lists the ones already published.

The `publish` input (default `true`) gates the release job, which also needs a tag ref. With `publish: false` the plan and dry run run on any ref and nothing is published. The `release.yml` that `dx init` writes has a `workflow_dispatch` with `publish` defaulting to `false`, so dispatching it from a branch only plans and dry-runs; pass `true` on a tag to publish. Tag pushes always publish. An existing `release.yml` keeps its file: add the `workflow_dispatch` `publish` input and `publish: ${{ github.event_name != 'workflow_dispatch' || inputs.publish }}` under `with:` by hand.

| Input | Meaning |
|---|---|
| `deployables` | JSON array of labels, for example `["//pkg:npm", "//pkg:github"]`. Empty publishes every `dx_deployment` tagged `release`. |
| `runner` | Runner label. Default `ubuntu-latest`. |

The caller grants `contents`, `id-token`, `attestations` and `packages` write and passes `secrets: inherit`, so registry tokens such as `NPM_API_TOKEN` are repository secrets. A `github_deploy` creates a draft release.

## Repository settings

- Approval policy: `all_external_contributors`.
- Merge gate: require up-to-date branches, or use a merge queue.
- Workflow permissions: read-only default.
