# GitLab CI example

```sh
cp .gitlab-ci.yml <your-repo>/.gitlab-ci.yml
```

Starter pipeline for GitLab. It includes the rules_dx template
`ci/gitlab/dx.gitlab-ci.yml` at a pinned commit. To update, change the commit
in the `include:` URL. The template runs four jobs on Linux runners: `check`,
`build`, `test` and `coverage`. Each job runs one `dx` line:

```sh
bazel run @rules_dx//:dx -- check --report sarif=reports/dx.sarif //...
bazel run @rules_dx//:dx -- build --report failures=reports/failures.json //...
bazel run @rules_dx//:dx -- test --report junit=reports/junit.xml //...
bazel run @rules_dx//:dx -- coverage --report lcov=reports/coverage.lcov //...
```

Not included, compared with the GitHub reusable workflow
(`.github/workflows/reusable-consumer.yml`):

- `--affected` on merge requests. The GitHub input is `affected_on_pr`.
- Platforms other than Linux x86_64. The GitHub input is `platforms`.
- Coverage gates. The GitHub inputs are `min_coverage` and
  `min_changed_line_coverage`.
- The `security-audit` and `license-audit` checks and the advisory snapshots.
  The GitHub inputs are `disabled_checks` and `invocation_id`.
- A fallback cache key. The GitHub workflow has no input for it. It restores
  from a key prefix when the exact key misses.

To run the same steps on another CI provider:

1. Install Bazelisk as `bazel` and check its SHA-256.
2. Commit `.bazelversion`.
3. Set `XDG_CACHE_HOME` to `.cache` inside the checkout.
4. Run `mkdir -p reports`, then the `dx` lines above.

## Consumer setup

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

Opt in with a GitLab CI/CD variable:

1. Add a variable named `DX_REMOTE_BAZELRC` of type File. Mark it
   protected. Put your remote cache flags in it. GitLab cannot mask a
   multi-line value, so never print the file in a job.
2. Add this line to the consumer `.bazelrc`:

```text
try-import %workspace%/.cache/remote.bazelrc
```

Only protected branches get the file. Merge request pipelines run without
it.

## Image and runner

The template sets a digest-pinned `buildpack-deps:noble-scm` image. Jobs run
on the default GitLab runner. Override `default:` in your `.gitlab-ci.yml` to
change it.
