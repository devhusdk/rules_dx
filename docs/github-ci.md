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

Every job runs `bazel run @rules_dx//tools/ci:module_pin_bin` before `dx`.
It checks the `rules_dx` declaration in `MODULE.bazel`. Pass that same version as `rules_dx_version`. A repository that
calls the workflow from its own tree declares itself instead:

```python
module(
    name = "rules_dx",
    version = "0.0.0",
)
```

Bazel resolves `rules_dx` first. It stops with its own error before the
check runs if `MODULE.bazel` declares no `rules_dx`, declares it twice, or
pins a release without `//tools/ci:module_pin_bin`.

- A multiline call, reordered arguments and comments are fine.
- A commented-out pin never counts.
- Only the `rules_dx` declaration counts.
- A mismatched version, or one that is not a quoted string, fails with the
  line, the declared version and the expected one.

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
  `min_coverage` is set. With `min_changed_line_coverage` it also writes the
  LCOV report and gates the lines a pull request adds.

`--check` applies to the four quality checks. `dx security`, `dx license`,
`dx test`, `dx build`, and `dx coverage` reject it.

The `validate (inputs)` job runs first. It reads every input, rejects a bad one with the
accepted values, and starts no check.

Inputs:

- `rules_dx_version`: the version your `rules_dx` declaration carries. The
  pin check compares it with `MODULE.bazel`.
- `disabled_checks`: job IDs above to skip, comma-separated. Empty runs all
  nine. Surrounding spaces are ignored. An unknown ID, a repeated ID and an
  empty ID fail.
- `platforms`: JSON array of platform labels, such as `["linux_x86_64"]`.
  Omit it to run `["linux_x86_64"]`. Valid labels: `linux_x86_64`, `linux_arm64`,
  `macos_arm64`, `windows_x86_64`, `windows_arm64`. Malformed JSON, a JSON value
  that is not an array, an empty array, a repeated label, an unknown label and a
  label carrying spaces all fail.
- `scheduling_mode`: `parallel` only. `sequential` fails; checks always run in
  parallel.
- `code_scanning_opt_in`: `false` only. `true` fails; no SARIF upload step ships
  in this workflow.
- `min_coverage`: fails below that percent, a whole number from 0 to 100 such as
  `80`. Empty collects without enforcing a threshold. `80%`, `80.5`, `-1`, `101`,
  `eighty` and a leading space fail. Coverage comes from `dx coverage`.
- `min_changed_line_coverage`: fails when fewer than that percent of the
  executable lines a pull request adds are covered, a whole number from 0 to
  100. Empty skips the check. It accepts and rejects the same values as
  `min_coverage`, and it never changes `min_coverage`.
- `invocation_id`: 1 to 40 letters, digits, underscores or hyphens. Empty
  uploads `advisory-snapshots`, which suits one call per workflow run. Two calls
  in one workflow run, including matrix cells, each need their own ID, such as
  `api-linux`; a repeated ID makes the second upload fail. Anything else fails.
  The ID also keeps the calls' concurrency groups apart, so one call never
  cancels another. A rerun reuses the same ID.

## Changed-Line Coverage

Set `min_changed_line_coverage` to require coverage on new code without raising
the whole-repo threshold. The `coverage` job fetches full history, runs
`dx coverage --report lcov=<path>`, then runs `diff-cover` 10.6.0 over that
report. It runs on `pull_request` events only. Other events print a notice and
skip it.

The base is the pull request base commit. The head is the checked out `HEAD`.
Lines come from the diff between their merge-base and `HEAD`. Renames follow
the file, so a pure rename adds no lines. Deleted lines are not counted. Report
paths are mapped to workspace-relative paths and must match a changed file
exactly. An absolute path maps to the longest tracked path it ends with.

The check prints the base, merge-base and head commits, the threshold, the
covered and eligible line counts, whether the evidence is complete, and each
uncovered `path:line`. Lines the report does not list, such as comments and
docs, are not eligible. With no eligible line the result is `not-applicable`,
not 100 percent.

It fails when:

- fewer than the threshold percent of eligible lines are covered. The compare is
  exact, with no rounding.
- the base commit is not available, or the history is too shallow to reach a
  merge-base.
- the report is missing, empty, or not valid LCOV.
- the `diff-cover` output lacks its line totals or per-file stats.
- a changed file has the extension of files in the report but is not in it.
  The result is `incomplete`.

```yaml
    with:
      rules_dx_version: "0.0.0"
      min_changed_line_coverage: "80"
```

A rejected input fails in the `validate` job before any check starts. The nine
checks need it, so none of them run. `dx-ci (aggregate)` names the validation
failure and fails with them.

`dx-ci (aggregate)` passes only when every check succeeds. It fails when a check
fails or is cancelled, when a declared check never reports a result, and when a
check is skipped without being listed in `disabled_checks`. A check you disabled
stays distinguishable from a check that never ran.

The workflow needs `contents: read` plus `checks: write` for check runs.
`coverage` also needs `pull-requests: write` for its pull-request comment.
The comment shows PASS or FAIL for the coverage threshold and the changed-line
gate, and every pull-request run updates it, including failed runs.
Pull requests use the read-only cache config automatically.

A called workflow can only narrow the calling job's token, so grant those
scopes on the calling job. `examples/consumer-ci/caller.yml` does.

## Evidence Workflow

`evidence.yml` runs Bazel target patterns on hosted runners for native evidence
this repository cannot produce locally. Dispatch it per run; it never runs on
push:

```sh
gh workflow run evidence.yml --ref <branch> -f scope=//cli/cli/...
gh workflow run evidence.yml --ref <branch> -f scope="//cli/process/... //tools/testing/process_probe/..."
```

- `scope`: space-separated Bazel target patterns. Each must start with `//`.
- `task`: `test` (default) or `build`.
- `platforms`: JSON array of platform labels. Accepted: `linux_x86_64`,
  `linux_arm64`, `macos_arm64`, `windows_x86_64`, `windows_arm64`.
- `test_timeout`: seconds each test may run. Default `600`.
- `displays`: Linux only. `xcb`, `wayland` or `xcb,wayland`. Starts Xvfb and a
  headless Weston and sets `DX_QT_DISPLAYS`. Default empty: no displays.

A `validate` job rejects a bad input naming the accepted values. Each platform
cell runs the task, uploads its log as the `evidence-<platform>` artifact, and
writes the tail to the job summary.

## Apple Workflow

`apple.yml` runs the Apple toolchain tests on a hosted macOS runner with Xcode
preinstalled. Dispatch it per run:

```sh
gh workflow run apple.yml --ref <branch> -f command=test \
  -f targets="//apple/... //apple/tests:macos_consumer_test //apple/tests:macos_app_consumer_test //apple/tests:macos_framework_consumer_test"
```

- `command`: `test` (default) or `build`.
- `targets`: space-separated Bazel targets. Each must start with `//`.
- `runner`: `macos-15` (arm64, default).

Each run uploads the
`apple-evidence-<runner>` artifact with the executor's macOS, Xcode, and SDK
identities, the Bazel log, and the test logs and outputs.

## Advisory Snapshots

`security-audit` reads `.dx/advisory/`. One
`advisory snapshots (one run, every platform)` job fills that directory once
per run, uploads it as the `advisory-snapshots` artifact
(`advisory-snapshots-<invocation_id>` when set), and every platform cell
downloads the same
bytes. No cell downloads a database of its own.

```sh
bazel run @rules_dx//cli/advisory_prep --
```

The command converts one archive per advisory family the dependency locks need,
so a repository with only `Cargo.lock` prepares the `cargo` snapshot alone. It
writes `<out>/<family>.json` and `<out>/<family>.meta.json`. The sidecar holds
the set, the source URL, the `sha256` of the payload, the stamped date and the
payload path. The payload is the same for the same advisories whatever order
the archive lists them in.

Relative paths anchor at the workspace root.

- `--workspace <dir>`: dependency set locks to read. Default the workspace root.
- `--out <dir>`: where the snapshots are written. Default `.dx/advisory` under it.
- `--date YYYY-MM-DD`: date every sidecar carries. Default today, UTC.
- `--max-time <seconds>`: wall clock limit for one download. Default 600.
- `--connect-timeout <seconds>`: connect limit for one download. Default 30.
- `--retries <count>`: attempts one download may spend. Default 3.
- `--retry-delay <seconds>`: pause between two attempts. Default 5.
- `--archive FAMILY=PATH`: convert a local archive instead of fetching it.
  Repeatable.
- `--status-file <file>`: append `status`, `ecosystems` and `families` as
  `key=value` lines. The workflow passes `$GITHUB_OUTPUT`.

A repository whose locks need no advisory ecosystem prepares nothing. The
command prints `no advisory ecosystems needed`, exits `0` and reports
`status=empty` and `ecosystems=0`. The job then uploads no artifact, the
`security-audit` cells download none, and secret scanning still runs. Dependency
sets without a lock are not audited. A missing artifact for a needed ecosystem
still fails the audit and `dx-ci (aggregate)`.

Only `https` and `file` URLs are fetched, a redirect stays on `https`, and an
archive entry is refused when its length or checksum disagrees with the
archive. Exit codes: `0` success, `2` usage error, `1` preparation failure. A
failure prints one line starting with `advisory_prepare_failed:` naming the
family and the archive entry, path or limit it could not use, and writes no
snapshot for that family.

The artifact is kept for one day. Each `security-audit` cell judges freshness
against the date the job stamped, passed to `dx security` as `DX_AUDIT_TODAY`,
so every cell accepts or rejects the same snapshot. `security-audit` needs the
job, so a failed preparation skips every cell and `dx-ci (aggregate)` fails. A
missing, stale or corrupt snapshot fails the audit with `advisory_refresh_failed`
naming the set and the file. The job needs `contents: read` and no secret, so
fork pull requests run it.

## Hermetic Tool Resolution

`security-audit` scans secrets with the pinned Gitleaks artifact, and `dx`
refuses ambient `PATH` lookup. One `Resolve hermetic Gitleaks` step resolves
the artifact path with a native command instead of shell parsing, so every
platform cell exports the same variable without host utilities.

```sh
bazel run @rules_dx//cli/tool_resolve -- --label @dx_tools//:gitleaks --var DX_GITLEAKS_BIN
```

The command runs `bazel cquery --output=files` for the label, takes the last
output line as the artifact path, joins it with `bazel info output_base`, and
refuses a missing file. It appends `VAR=path` to the file `GITHUB_ENV` names.

- `--label <label>`: pinned tool label to resolve. Required.
- `--var <name>`: environment variable receiving the absolute path. Required.
- `--bazel <path>`: Bazel binary the resolution runs through. Default `bazel`.
- `--workspace <dir>`: directory the inner Bazel commands run in. Default
  `BUILD_WORKSPACE_DIRECTORY`, the `bazel run` invocation root, else the
  current directory.
- `--env-file <path>`: environment file receiving `VAR=path`. Default
  `GITHUB_ENV`.
- `--print`: write `VAR=path` to standard output instead of a file.
- Arguments after `--` reach the inner cquery unchanged.

Exit codes: `0` success, `2` usage error, `1` resolution failure. A failure
prints one line starting with `tool_resolve_failed:` naming the label and the
cquery, output-base, missing-file or environment write it could not use, and
exports nothing.

In a local shell, export the same variable with `--print`:

```sh
export "$(bazel run @rules_dx//cli/tool_resolve -- --label @dx_tools//:gitleaks --var DX_GITLEAKS_BIN --print)"
```

## Docs Workflow

`examples/docs-ci/` calls `reusable-docs.yml`. It runs `dx lint --check` over
`docs_scope` (default `//...`) plus `dx docs --check` site validation, then
renders the site with `dx docs` and checks the rendered tree for missing
pages, styles, and search index, so broken links fail before deploy.

Inputs:

- `rules_dx_version`: the version your `rules_dx` declaration carries. The
  pin check compares it with `MODULE.bazel`.
- `docs_scope`: target patterns covered by the docs check, separated by
  spaces, such as `//... //docs:all`. Default `//...`. Each pattern reaches
  `dx` as one argument. A value with quotes, shell characters, wildcards, a
  leading `-` or anything but a target pattern fails with one error naming it.
- `publish`: build the rendered site and deploy to Pages. Default false.
- `environment`: Pages environment name. Default `github-pages`.

Publishing needs `pages: write` plus `id-token: write`. The publish job stages
and validates the whole rendered tree, deploys it, then loads the reported
page URL and checks routes, assets, and search before the clean-checkout
proof. Enable Pages with source GitHub Actions before the first publishing
run.

## Bump Workflow

`.github/workflows/bump.yml` opens one dependency PR per run.

Create a GitHub App with `contents: write` and `pull-requests: write`.
Install it on the repository. Set two secrets:

- `BUMP_APP_ID`: the App ID.
- `BUMP_APP_PRIVATE_KEY`: the App private key.

Dispatch the workflow with `selector` and `version`:

```sh
gh workflow run bump.yml -f selector=cargo:anyhow -f version=1.2.3
```

A run without both inputs lists outdated dependencies and changes nothing.

A dispatched run fails before changing files when the secrets are missing.
It pushes the branch and opens the PR as the App, so the PR runs the
required checks. Only the manifest, lock and pin files for the selected set
are staged. Other changed files fail the run. One run never batches two
dependencies. Set `automerge` to `true` to merge on the full required-check
set.

## Repository Settings

Set these once in the repository. Each caller already grants its job
permissions, so a read-only repository default is fine.

- Approval policy: `all_external_contributors`, so fork pull requests run.
- Branch protection for the consumer workflow: require `dx-ci (aggregate)`.
  One green gate covers the whole matrix.
- Branch protection for the docs workflow: require `docs-check`. Add
  `docs-publish (GitHub Pages)` once `publish` is true.
- Merge gate: require branches to be up to date, or use a merge queue. Both
  callers run on `merge_group` (`checks_requested`), so the queue gets the
  required checks for its own SHA. Merge-group runs post no PR comment and
  never publish Pages.
- Pages source: GitHub Actions, before the first publishing run.
- Workflow permissions: read-only by default. Each calling job then grants
  what its reusable jobs request: the consumer caller `checks: write` plus
  `pull-requests: write`, the docs caller `pages: write` plus
  `id-token: write`.

## Caching

Both callers forward repository secrets with `secrets: inherit`. Set
`BUILDBUDDY_API_KEY` to share the BuildBuddy remote cache. Codecov is
optional.

Every job restores the Bazel repository cache that Bazel and `dx` use. Each
job and platform keeps its own copy, keyed on `MODULE.bazel` and
`MODULE.bazel.lock`. Pushes save it. Pull requests, forks included, only
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
