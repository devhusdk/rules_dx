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

- `dx update`: set selectors. No selector updates every set.
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
- `--here`: the current directory tree (`//path/...`,
  `//...` at the root). Never combines with explicit scopes. Accepted by
  `dx build`, `dx test`, `dx coverage`, `dx lint`, `dx typecheck`,
  `dx format`, `dx generate`, `dx check`, `dx fix`, `dx security`,
  `dx license`, `dx list-tests`, and `dx docs`.
- `--affected`: the targets your changes reach, described below. Never
  combines with explicit scopes or `--here`.

```sh
bazel run @rules_dx//:dx -- lint --here
bazel run @rules_dx//:dx -- build //cli/...
```

## `--affected`

`--affected` selects only what your changes can reach. It is accepted by
`dx build`, `dx test`, `dx coverage`, `dx list-tests`, `dx lint`,
`dx typecheck`, `dx format`, `dx check`, `dx fix`, `dx security`, and
`dx license`. `dx generate` and `dx docs` always cover the whole repo and
reject it.

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
  tests and only reports fixes; `dx fix --apply --affected` writes them. Generate always covers the whole repo.
- A change to `MODULE.bazel`, `MODULE.bazel.lock`, `.bazelrc`,
  `.bazelversion`, or any lock file widens the scope to `//...`. A deleted,
  removed, or renamed-from file also widens to `//...`. An unowned file
  widens to its nearest package.
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
  `dx: affected: 4 targets from 1 changed files since 1a2b3c4d5e6f; widened to //app:all (loads tools/defs.bzl)`.
- No changes, or no affected targets, prints `no affected targets` on stdout
  and exits `0`. With `--output=json` it emits `command_started` (with
  `affected`) and `command_finished`; `mode` is `check` unless `--apply` is
  given.
- `--affected` takes no value. It never combines with explicit scopes or
  `--here`: both are usage errors with exit `2`.
- `--output=json` adds `affected` to the `command_started` event:
  `{"base": "<sha>", "changed_files": [...], "targets": [...], "widened": [{"scope": "//...", "reason": "..."}]}`.
  `base` is the merge-base commit, `targets` is the selection, and `widened`
  is empty when nothing widened. A changed `.bzl` file adds one `//pkg:all`
  entry per loading package.
- A checkout without a default branch, or with shallow history, fails with
  exit `2` and says to fetch full history or pass explicit scopes.
- dx runs one or two `bazel query` calls to find the owning targets of the
  changed files, one to expand reverse dependencies, and one more for
  `dx check --affected` to find its tests. It keeps no daemon and no cache
  files.
- `dx security --affected` always runs the secrets scan, even when no
  dependency set owns a reached target.

```sh
bazel run @rules_dx//:dx -- test --affected
bazel run @rules_dx//:dx -- check --affected --output=json
```

