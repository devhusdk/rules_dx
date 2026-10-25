# Scope Defaults

Most `dx` commands take Bazel scopes. This page lists what each command does
with a scope.

## No Scope Means `//...`

- `dx build`, `dx test`, `dx coverage`, `dx lint`, `dx typecheck`,
  `dx format`, `dx check`, `dx fix`, `dx security`, `dx license`, `dx migrate`,
  `dx list-tests`

## No Scope Means The Repository

- `dx generate`: runs Gazelle over the whole repo.
- `dx docs`: builds `//tools/docs/site:user_site`, or
  `//tools/docs/site:user_site_check` in `--check`.

## Repository-Wide Or One Exact Label

- `dx env`, `dx codegen`

Patterns, paths, and multiple labels are usage errors.

## Set Selectors

- `dx update`: set, `set:package`, label, or path selectors. No selector
  checks every set; `--apply` writes.
- `dx bump`: exactly one `set:package` plus one version.

## Required Arguments

- `dx run`: needs a runnable scope.
- `dx deploy`: exactly one main-workspace label.
- `dx watch`: needs a watchable command plus its scopes.
- `dx new`: needs a language, plus an optional name.
- `dx init`: optional module name.
- `dx completion`: exactly one shell.

## No Scopes

- `dx clean`, `dx status`, `dx version`, `dx upgrade`, `dx rollback`, `dx worktree`

## Scope Shapes

- Labels and patterns: `//...`, `//pkg:target`, `@repo//...`.
- Files and dirs: workspace-relative paths resolved through `bazel query`
  to the owning targets. The ownership query covers the whole workspace, so
  owners in other packages are included.
- Paths with control characters in a file or directory name are refused.
- `--here`: the current directory tree, or `BUILD_WORKING_DIRECTORY` under
  `bazel run` (`//path/...`, `//...` at the root). Never combines with
  explicit scopes. Accepted by `dx build`, `dx test`, `dx coverage`,
  `dx lint`, `dx typecheck`, `dx format`, `dx generate`, `dx check`,
  `dx fix`, `dx security`, `dx license`, `dx list-tests`, and `dx docs`.
- `--affected`: the targets your changes reach, described below. Never
  combines with explicit scopes or `--here`.
- Targets that do not fit the target platform are skipped in `dx build`,
  `dx test`, `dx coverage`, `dx lint`, `dx typecheck`, `dx format`,
  `dx check`, and `dx fix`, even when named, reached through files, or picked
  by `--affected`. Skipped tests report the outcome `skipped`. Pass
  `-- --noskip_incompatible_explicit_targets` to fail on them instead.
  `dx run` still fails on them.

```sh
bazel run @rules_dx//:dx -- lint --here
bazel run @rules_dx//:dx -- build //cli/...
```

## `--affected`

`--affected` selects only what your changes can reach. It is accepted by
`dx build`, `dx test`, `dx coverage`, `dx list-tests`, `dx lint`,
`dx typecheck`, `dx format`, `dx check`, `dx fix`, `dx security`, and
`dx license`. `dx generate` and `dx docs` reject `--affected` (usage error,
exit `2`).

- Changes are the diff from the merge-base with the default branch
  (`origin/HEAD`, else `origin/main`, `main`, `origin/master`, `master`), plus
  staged, unstaged, and untracked files. A rename counts both paths.
- When the workspace is a subdirectory of the Git repo, paths are relative to
  the workspace and changes outside it are ignored. A change under a
  `local_path_override` path outside the workspace widens to `//...`.
- Each changed file maps to its owning targets, then to their reverse
  dependencies inside `//...`. Targets tagged `manual` are left out.
- `dx test`, `dx coverage`, and `dx list-tests` keep only test targets.
- `dx security` and `dx license` keep only targets a dependency set owns.
- `dx check --affected` runs format, lint, typecheck, and generate as usual,
  then the affected tests. `dx fix --affected` runs the same phases without
  tests and only reports fixes; `dx fix --apply --affected` writes them.
  Generate and its re-check run Gazelle on the directories of the affected
  targets.
- A change to `MODULE.bazel`, `MODULE.bazel.lock`, `WORKSPACE`,
  `WORKSPACE.bazel`, `.bazelversion`, or any `.bazelrc*` file, at any depth,
  widens the scope to `//...`. A deleted, removed, or renamed-from file also
  widens to `//...`. An unowned file, or one outside any package, widens to
  the package tree around it, such as `//pkg/...`.
- A changed `BUILD` or `BUILD.bazel` file selects every target in its package,
  plus its reverse dependencies. An added `BUILD` file widens to `//...`; a
  deleted one widens to `//...` like any deleted file.
- A change to a `Cargo.lock` or crate_universe `lockfile` named by a
  `crate.from_cargo` call in `MODULE.bazel` selects the targets that use the
  changed crates, plus the crates that depend on them, and their reverse
  dependencies. Any other `*.lock` file, an added or deleted lock, a lock dx
  cannot read, or a crate_universe lockfile change outside `crates` widens
  to `//...`.
- A changed `.bzl` file selects the packages whose BUILD files load it,
  directly or through other `.bzl` files, plus their reverse dependencies.
  dx reads the BUILD and `.bzl` files to find them. Targets that own the
  `.bzl` file, such as a `bzl_library`, are selected too. It widens to
  `//...` when the file is used by a `MODULE.bazel` extension or repo rule,
  by an aspect named in `.bazelrc` or one dx runs, or when a BUILD or `.bzl`
  file does not parse. A `.bzl` file nothing loads selects nothing.
- dx prints each widening and its reason on stderr:
  `dx: affected: 12 targets from 3 changed files since 1a2b3c4d5e6f; widened to //... (workspace input MODULE.bazel)`
  or
  `dx: affected: 4 targets from 1 changed files since 1a2b3c4d5e6f; widened to //app:all (loads tools/defs.bzl)`
  or
  `dx: affected: 4 targets from 1 changed files since 1a2b3c4d5e6f; widened to //app:all (build file app/BUILD.bazel)`.
- Before the queries run, dx prints
  `dx: affected: resolving changes since 1a2b3c4d5e6f` on stderr, then the
  summary. `--quiet`, `--output=diff`, and `--output=json` print neither. An
  empty selection prints no summary.
- No changes, or no affected targets, prints `no affected targets` on stdout
  and exits `0`. With `--output=json` it emits `command_started` (with
  `affected`) and `command_finished`; `mode` is `check` unless `--apply` is
  given.
- `--affected` takes no value. It never combines with explicit scopes or
  `--here`: both are usage errors with exit `2`.
- `--output=json` adds `affected` to the `command_started` event:
  `{"base": "<sha>", "changed_files": [...], "targets": [...], "tests": [...], "widened": [{"scope": "//...", "reason": "..."}]}`.
  `base` is the merge-base commit, `targets` is the selection, and `widened`
  is empty when nothing widened. A changed `.bzl` file adds one `//pkg:all`
  entry per loading package. A lock file change adds one entry per lock with
  `repos`, the changed crates:
  `{"scope": "rdeps(//..., @crates//:serde)", "reason": "changed crates in Cargo.lock", "repos": ["@crates//:serde"]}`.
  With more than five crates the scope reads `rdeps(//..., <n> repos)`.
  `tests` appears only for `dx check --affected`. It lists the tests the
  last phase runs: `//...` when widened, empty when no test is affected. dx
  writes `command_started` only after planning finishes.
- A checkout without a default branch exits `2` with
  `cannot find the default branch: expected origin/HEAD, main, or master`.
- A shallow checkout exits `2` with
  ``git history is shallow: fetch full history with `git fetch --unshallow` or pass explicit scopes``.
- A failed `bazel query` exits `2`. The `error` event has code
  `query_failed` and keeps Bazel's exit status and output excerpt. A query
  that cannot start has code `query_launch_failed`.
- dx runs at most one `bazel query`, `kind('rule', //...)`, to find the
  owning targets of the changed files and their reverse dependencies.
  `dx test`, `dx coverage`, `dx list-tests`, and `dx check` then run one
  `tests(set(...))` query to find the affected tests. A change that widens
  to `//...` skips the tests query.
- `dx security --affected` always runs the secrets scan, even when no
  dependency set owns a reached target.

```sh
bazel run @rules_dx//:dx -- test --affected
bazel run @rules_dx//:dx -- check --affected --output=json
```

