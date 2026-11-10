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
Read `docs/design/layout.md` before you add or move a language.

- Treat warnings as errors.
- Run the formatter, linter, and focused tests for your change.
- Update tests and user docs when behavior changes.
- Do not edit generated files. Use the generator.
- Library crates return typed errors. Only binaries map exit codes.

## Add a language

```sh
bazel run //tools/new_language -- <lang> [title]
```

It creates `<lang>/` with `defs.bzl`, `extensions.bzl`, `BUILD.bazel`, `lang.json`, `pins.json`, `private/` and `tests/`, and adds the directory to `core/modules/families.json`. Fill in `lang.json`, then run `bazel run //tools/docs/support_matrix:update`.

## Remote cache

Export your BuildBuddy API key and add `--config=bb`:

```sh
export BUILDBUDDY_API_KEY=<key>
tools/dx test --affected -- --config=bb
```

`--config=bb` reads the key from the environment through `tools/buildbuddy-credential-helper.sh` on Linux and macOS. Never put the key in `user.bazelrc` or any other rc file, and never pass `--announce_rc`.

A read-only key makes Bazel warn on every build that it cannot upload results. Use `--config=bb-ro` instead of `--config=bb`.

## Quality tests

- `bazel test //quality/testdata:real_results`
- `bazel test //quality/testdata:tool_overrides`
- `bazel test //quality/testdata:runner_matrix`
- `bazel test //quality:all`

## Evidence status

Each `evidence.yml` run posts the commit status `evidence/<platform>/<task>/<scope>` on the dispatched commit: pending at dispatch, then success, failure or error. `gh pr checks <n> --watch` shows it. A second run for the same platform, task and scope on one commit replaces the status. The merge-group `gate` job fails when the newest status of any `evidence/*` context on the pull request head is `pending`, `failure` or `error`.

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
versions in `core/modules/versions.bzl`:

- `<DEP>_VERSION` when the floor and the tested version are the same.
- `FLOOR_<DEP>` and `TESTED_<DEP>` when they differ.

The floor equals the tested version for every consumer dep (#6016): only the
tested versions are supported. `//core/modules:consumer_deps_test` fails when they differ.

A `# floor: equals tested` comment directly above each `bazel_dep` marks this.
Tools parse that marker, so keep it as written. To bump a dep, set the `bazel_dep`
and the `single_version_override` to the same version:
`dx update bazel:<dep> <version>` and
`dx update bazel:<dep> <version> --floor --reason "equals tested"`.

### Where floors are checked

rules_dx is a non-root module in every consumer, so its root-only
`single_version_override`s do not apply there and a consumer resolves the floors.
`//core/modules:consumer_deps_test` keeps the hermetic consumer `module_fragment.bazel`
files from pinning a consumer dep above its floor.

`.dx/local/launcher` is a copy (about 50 MB) of the last built dx, kept for worktree hooks and failed rebuilds. `tools/dx worktree add`, `release` and `hook` run the last built dx without rebuilding, so they never wait on Bazel. Any other `tools/dx` command (or `bazel build //:dx`) picks up dx source changes. You can delete it any time and `tools/dx` recreates it. `tools/dx worktree prune --apply --bases` drops it in free slots. Both launchers run dx from this copy, so a running dx never locks bazel-bin; `dx.exe.old.*` files may remain while an old dx is still running.
