# GitLab CI example

```sh
cp .gitlab-ci.yml <your-repo>/.gitlab-ci.yml
```

Starter pipeline for GitLab. It runs four jobs on Linux runners: `check`,
`build`, `test` and `coverage`. Each job runs one `dx` line:

```sh
bazel run @rules_dx//:dx -- check --report sarif=reports/dx.sarif //...
bazel run @rules_dx//:dx -- build --report failures=reports/failures.json //...
bazel run @rules_dx//:dx -- test --report junit=reports/junit.xml //...
bazel run @rules_dx//:dx -- coverage --report lcov=reports/coverage.lcov //...
```

The same recipe works on any CI provider:

1. Install Bazelisk 1.29.0 as `bazel` and check its SHA-256.
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

The pipeline uses `buildpack-deps:noble-scm@sha256:59d4f4d8581d741bb2c31f4bfd00a557ca94d182d082c81bd56d32afbad8dfc9`
on the `saas-linux-small-amd64` runner.
