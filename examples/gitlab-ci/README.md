# GitLab CI example

`dx init --ci gitlab --apply` writes a `.gitlab-ci.yml` that includes the rules_dx template
`ci/gitlab/dx.gitlab-ci.yml` at a pinned commit. To update, change the commit in the
`include:` URL. The template runs four jobs on Linux runners: `check`,
`build`, `test` and `coverage`, plus `fuzz` and `bench` when their inputs are
set. Each of the four runs one `dx` line. On merge requests, `check`, `build`, `test` and `coverage` use `--affected` instead of `//...`:

```sh
bazel run @rules_dx//:dx -- check --report sarif=reports/dx.sarif //...
bazel run @rules_dx//:dx -- build --report failures=reports/failures.json //...
bazel run @rules_dx//:dx -- test --report junit=reports/junit.xml //...
bazel run @rules_dx//:dx -- coverage --report lcov=reports/coverage.lcov //...
```

## Inputs

The template takes the inputs of the GitHub reusable workflow
(`.github/workflows/reusable-consumer.yml`) under the same names. Pass them
in the `include:` entry:

```yaml
include:
  - remote: https://raw.githubusercontent.com/devhusdk/rules_dx/<commit>/ci/gitlab/dx.gitlab-ci.yml
    inputs:
      min_coverage: "80"
      fuzz: true
```

| Input | Meaning |
|---|---|
| `affected_on_pr` | Default `true`. On merge requests, `check`, `build`, `test` and, without a coverage threshold, `coverage` use `--affected`. |
| `buf_breaking_on_pr` | Default `false`. On merge requests, `check` also runs `dx lint --breaking-against`. |
| `link_check_online` | Default `false`. Outside merge requests, `check` also runs `dx lint --online //...`. |
| `min_coverage` | Whole number 0 to 100. Fails below it. Empty sets no threshold. |
| `min_changed_line_coverage` | Whole number 0 to 100. Merge requests only. Empty skips it. |
| `fuzz` | Default `false`. Adds a `fuzz` job that runs `dx fuzz` on `dx-fuzz` targets. The log is `reports/fuzz.log`; a crash fails the job and keeps `.dx/local/fuzz` under `reports/`. |
| `fuzz_time` | Default `300s`. Time budget per fuzz target: `90s`, `5m` or `1h`. |
| `bench` | Default `false`. Adds a `bench` job that runs `dx bench`. On merge requests it first benchmarks the merge base in a worktree under `.cache/`, then runs `dx bench --compare` against it. The table is `reports/bench.txt` and the results are `reports/last.json`. |
| `bench_max_regression` | Default `20`. Percent a benchmark may be slower than the merge base before the `bench` job fails. |

On merge requests the template fetches the target branch and points
`origin/HEAD` at it, so `--affected`, `--breaking-against` and changed-line
coverage find the merge base.

Not included, compared with the GitHub reusable workflow:

- Platforms other than Linux x86_64. The GitHub input is `platforms`.
- The `security-audit` and `license-audit` checks and the advisory snapshots.
  The GitHub inputs are `advisory_pr` and `invocation_id`.
- A `remote_cache` input. Set the `DX_REMOTE_CACHE` variable below.
- A fuzz corpus cache. Each `fuzz` job starts from the checked-in seeds.
- A fallback cache key. The GitHub workflow has no input for it. It restores
  from a key prefix when the exact key misses.

## Other CI providers

To run the same steps on another CI provider:

1. Install Bazelisk as `bazel` and check its SHA-256.
2. Commit `.bazelversion`.
3. Set `XDG_CACHE_HOME` to `.cache` inside the checkout.
4. Run `mkdir -p reports`, then the `dx` lines above.

## Consumer setup

rules_dx is not released yet. Install starts with the first BCR release.

Declare the module in `MODULE.bazel`:

```python
bazel_dep(name = "rules_dx", version = "0.0.0")
```

Commit `MODULE.bazel.lock` and `.bazelversion`. The cache key reads both.

Add to `.gitignore`:

```text
.cache/
reports/
bazel-*
.dx/
```

Add to `.bazelignore`:

```text
.cache
```

## Reports

Every job keeps `reports/` as an artifact, also when the job fails. A job
that hits its timeout keeps nothing.

- `reports/junit.xml` shows in the merge request test widget.
- `reports/coverage.lcov`, `reports/dx.sarif` and `reports/failures.json`
  are plain artifacts. GitLab coverage reports take only Cobertura or
  JaCoCo, and the SARIF widget needs GitLab Ultimate.

## Exit status

A job exits with the `dx` exit code. When `dx` passes, the job still fails
with `1` if its report is missing or empty, or if the run changed tracked
or untracked files. The job prints the missing path or the changed files.

## Caching

Each job caches only the Bazel repository cache under `.cache/bazel/`. The
key is the job name plus `MODULE.bazel.lock` and `.bazelversion`. GitLab
keeps separate caches for protected and unprotected branches, so merge
request pipelines never write the cache that default branch pipelines read.
Bazel checks every cached file by its hash. Bazelisk is downloaded fresh in
every job.

## Remote cache

Opt in with GitLab CI/CD variables:

1. Add `DX_REMOTE_CACHE` with the cache URL, for example
   `grpcs://cache.example.com`. Mark it protected.
2. If the cache needs a header, add `DX_REMOTE_CACHE_HEADER` as `name=value`.
   Mark it protected and masked.

The template writes `common --remote_cache=...` and, with a header,
`common --remote_header=...` to the gitignored `user.bazelrc` before each job.
Only protected branches get the variables. Merge request pipelines run
without them.

## Image and runner

The template sets a digest-pinned `buildpack-deps:noble-scm` image. Jobs run
on the default GitLab runner. Override `default:` in your `.gitlab-ci.yml` to
change it.
