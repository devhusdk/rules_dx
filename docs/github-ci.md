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

Every job reads `MODULE.bazel` before it runs `dx` and checks the `rules_dx`
declaration. Pass that same version as `rules_dx_version`. A repository that
calls the workflow from its own tree declares itself instead:

```python
module(
    name = "rules_dx",
    version = "0.0.0",
)
```

The check reads the file as text. It never runs it.

- A multiline call, reordered arguments and comments are fine.
- A commented-out pin never counts.
- The version must be a quoted string.
- One declaration only. A second one fails.
- An unrelated dependency at the same version never passes.
- A missing, ambiguous or mismatched pin fails with the line, the declared
  version and the expected one.

The step needs `python3` on `PATH` on every platform.

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

A `validate` job runs first. It reads every input, rejects a bad one with the
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

A rejected input fails in the `validate` job before any check starts. The nine
checks need it, so none of them run. `dx-ci (aggregate)` names the validation
failure and fails with them.

`dx-ci (aggregate)` passes only when every check succeeds. It fails when a check
fails or is cancelled, when a declared check never reports a result, and when a
check is skipped without being listed in `disabled_checks`. A check you disabled
stays distinguishable from a check that never ran.

The workflow needs `contents: read` plus `checks: write` for check runs.
`coverage` also needs `pull-requests: write` for its pull-request comment.
Pull requests use the read-only cache config automatically.

A called workflow can only narrow the calling job's token, so grant those
scopes on the calling job. `examples/consumer-ci/caller.yml` does.

## Advisory Snapshots

`security-audit` reads `.dx/advisory/`. One `advisory snapshots (one run,
every platform)` job fills that directory once per run, uploads it as the
`advisory-snapshots` artifact, and every platform cell downloads the same
bytes. No cell downloads a database of its own.

```sh
bazel run @rules_dx//cli/advisory_prep --workspace . --out .dx/advisory
```

The command converts one archive per advisory family the dependency locks need,
so a repository with only `Cargo.lock` prepares the `cargo` snapshot alone. It
writes `<out>/<family>.json` and `<out>/<family>.meta.json`. The sidecar holds
the set, the source URL, the `sha256` of the payload, the stamped date and the
payload path. The payload is the same for the same advisories whatever order
the archive lists them in.

- `--workspace <dir>`: dependency set locks to read. Default `.`.
- `--out <dir>`: where the snapshots are written. Default `.dx/advisory`.
- `--date YYYY-MM-DD`: date every sidecar carries. Default today, UTC.
- `--max-time <seconds>`: wall clock limit for one download. Default 600.
- `--connect-timeout <seconds>`: connect limit for one download. Default 30.
- `--retries <count>`: attempts one download may spend. Default 3.
- `--retry-delay <seconds>`: pause between two attempts. Default 5.
- `--archive FAMILY=PATH`: convert a local archive instead of fetching it.
  Repeatable.

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

## Docs Workflow

`examples/docs-ci/` calls `reusable-docs.yml`. It runs `dx lint --check` over
`docs_scope` (default `//...`) plus `dx docs --check` site validation, so
broken links fail before deploy.

Inputs:

- `rules_dx_version`: the version your `rules_dx` declaration carries. The
  pin check compares it with `MODULE.bazel`.
- `docs_scope`: scope covered by the docs check. Default `//...`.
- `publish`: build the rendered site and deploy to Pages. Default false.
- `environment`: Pages environment name. Default `github-pages`.

Publishing needs `pages: write` plus `id-token: write`. Enable Pages with
source GitHub Actions before the first publishing run.

## Bump Workflow

`.github/workflows/bump.yml` opens one dependency PR per run. A weekly
schedule lists outdated dependencies. Dispatch the workflow with `selector`
(`set:package`, such as `cargo:anyhow`), `version`, and `automerge` (`true`
merges on the full required-check set, `false` leaves the PR open).

Create a GitHub App, install it on the repository, and set two secrets:

- `BUMP_APP_ID`: the App id.
- `BUMP_APP_PRIVATE_KEY`: the App private key.

Grant the App `Contents: read and write` plus `Pull requests: read and
write`. The workflow mints a token from the App just before pushing. The
branch push and the PR use that identity, so the PR schedules the ordinary
required CI. A run without both secrets fails before changing anything.

One run widens one requirement, refreshes its resolver-owned lockfile, runs
regen plus the build, test, coverage, lint, typecheck, format, security, and
license gates, then stages only that set's manifest and lock files. Changed
files outside the set, and any untracked file, fail the run instead of being
committed. Fork runs plan only and never push.

## Repository Settings

Set these once in the repository. Each caller already grants its job
permissions, so a read-only repository default is fine.

- Approval policy: `all_external_contributors`, so fork pull requests run.
- Branch protection for the consumer workflow: require `dx-ci (aggregate)`.
  One green gate covers the whole matrix.
- Branch protection for the docs workflow: require `docs-check`. Add
  `docs-publish (GitHub Pages)` once `publish` is true.
- Merge gate: require branches to be up to date, or use a merge queue.
- Pages source: GitHub Actions, before the first publishing run.
- Workflow permissions: read-only by default. Each calling job then grants
  what its reusable jobs request: the consumer caller `checks: write` plus
  `pull-requests: write`, the docs caller `pages: write` plus
  `id-token: write`.

## Caching

Both callers forward repository secrets with `secrets: inherit`. Set
`BUILDBUDDY_API_KEY` to share the BuildBuddy remote cache. Fork pull
requests run local-cache only. Codecov is optional.

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
