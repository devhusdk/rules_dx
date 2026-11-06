# Contributing

`rules_dx` is a Bazel developer platform. `dx` runs through Bazel.

```sh
tools/dx worktree add <branch>
tools/dx check --apply --affected
tools/dx test --affected
bazel run //dx:generate_check
tools/dx worktree release
```

Full run: `tools/dx test //...`

Read [AGENTS.md](AGENTS.md) before changing code.

- Treat warnings as errors.
- Run the formatter, linter, and focused tests for your change.
- Update tests and user docs when behavior changes.
- Do not edit generated files. Use the generator.
- Library crates return typed errors. Only binaries map exit codes.

## Issues and labels

Use the bug template. Paste `dx status`.

Each issue except pinned guides gets one priority label:

- `P0`: agent dev loop (worktrees, slots, fix, check, CI, launcher, JSON).
- `P1`: bugs in existing features.
- `P2`: improve and finish existing features.
- `P3`: new languages, platforms and features, last.

Other labels: `bug`, `enhancement`, `needs-host` (needs macOS, Windows, GPU, or signing secrets), `tracking`, `needs-review` (see below), `blocked` (waiting on something only the owner can do). A `blocked` issue starts a line of its body with `Blocked on:` followed by `#N` issues, `owner/repo#N` issues or pull requests in other repositories, a pull request or issue URL, or the owner action. Write each cross-repo reference in full: a bare `#N` always means this repository; the daily `issue-labels` workflow removes the label once every named issue is closed or pull request merged, and comments once on a `blocked` issue without the line. The `model:haiku`, `model:sonnet`, `model:opus` and `model:human` labels pick which agent lane handles an issue.
`dogfood` is an extra label, never the only one.

A pull request where `git diff --name-only origin/main...HEAD | tools/ci/review_paths.sh` prints paths (the prefixes live in `.github/review-paths.txt`) gets the `needs-review` label. It is not merged until a fresh session (the `reviewer` agent, not the author) posts a comment starting `Review:` that lists each finding as `file:line`, or says it found none. The daily `issue-labels` workflow warns about merged pull requests that skipped this.

Filters: [user issues](https://github.com/devhusdk/rules_dx/issues?q=is%3Aissue+is%3Aopen+-label%3Adogfood), [dogfood issues](https://github.com/devhusdk/rules_dx/issues?q=is%3Aissue+is%3Aopen+label%3Adogfood).

## Dependency floors

Every consumer-facing (non-dev) `bazel_dep` in the root `MODULE.bazel` has its
versions in `modules/versions.bzl`:

- `<DEP>_VERSION` when the floor and the tested version are the same.
- `FLOOR_<DEP>` and `TESTED_<DEP>` when they differ.

The floor equals the tested version for every consumer dep (#6016): only the
tested versions are supported. `//modules:consumer_deps_test` fails when they differ.

A `# floor: equals tested` comment directly above each `bazel_dep` marks this.
Tools parse that marker, so keep it as written. To bump a dep, set the `bazel_dep`
and the `single_version_override` to the same version:
`dx update bazel:<dep> <version>` and
`dx update bazel:<dep> <version> --floor --reason "equals tested"`.

### Where floors are checked

rules_dx is a non-root module in every consumer, so its root-only
`single_version_override`s do not apply there and a consumer resolves the floors.
`//modules:consumer_deps_test` keeps the hermetic consumer `module_fragment.bazel`
files from pinning a consumer dep above its floor.

`.dx/local/launcher` is a copy (about 50 MB) of the last built dx, kept for worktree hooks and failed rebuilds. `tools/dx worktree add`, `release` and `hook` run the last built dx without rebuilding, so they never wait on Bazel. Any other `tools/dx` command (or `bazel build //:dx`) picks up dx source changes. You can delete it any time and `tools/dx` recreates it. `tools/dx worktree prune --apply --bases` drops it in free slots. Both launchers run dx from this copy, so a running dx never locks bazel-bin; `dx.exe.old.*` files may remain while an old dx is still running.

Checkouts from before `.dx/local` can delete the old `.dx/ide` and `.dx/launcher` (`chmod -R u+w .dx/ide && rm -rf .dx/ide .dx/launcher`).
