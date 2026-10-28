# Scope Defaults

Most `dx` commands take Bazel scopes. This page lists what each command does
with a scope.

## No Scope Means `//...`

- `dx build`, `dx test`, `dx coverage`, `dx lint`, `dx typecheck`,
  `dx format`, `dx check`, `dx fix`, `dx security`, `dx license`, `dx migrate`

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
- `dx add`: exactly one undeclared `set:package` plus one version.

## Required Arguments

- `dx run`: needs a runnable scope.
- `dx deploy`: exactly one main-workspace label.
- `dx watch`: needs a watchable command plus its scopes.
- `dx init`: optional directory.
- `dx completion`: exactly one shell.

## No Scopes

- `dx clean`, `dx status`, `dx version`, `dx upgrade`, `dx rollback`, `dx worktree`, `dx explain`, `dx install`

## Scope Shapes

- Labels and patterns: `//...`, `//pkg:target`. `@repo//...` is accepted only by `dx run`; other commands reject external repos.
- Files and dirs: workspace-relative paths, also under `bazel run`. A file
  resolves through one `bazel query` of the workspace rules to the rules
  that use it directly. Owners in other packages are included. External
  repos are not loaded. A dir becomes `//dir/...`.
- Paths with control characters in a file or directory name are refused.
- `--here`: the current directory tree, or `BUILD_WORKING_DIRECTORY` under
  `bazel run` (`//path/...`, `//...` at the root). Never combines with
  explicit scopes. Accepted by `dx build`, `dx test`, `dx coverage`,
  `dx lint`, `dx typecheck`, `dx format`, `dx generate`, `dx check`,
  `dx fix`, `dx security`, `dx license`, and `dx docs`.
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
`dx build`, `dx test`, `dx coverage`, `dx lint`,
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
- `dx test` and `dx coverage` keep only test targets.
- `dx security` and `dx license` keep only targets a dependency set owns.
- `dx check --affected` runs generate, format, lint, and typecheck as usual,
  then the affected tests. `dx fix --affected` runs the same phases without
  tests and only reports fixes; `dx fix --apply --affected` writes them.
  Generate and its re-check run Gazelle on the directories of the affected
  targets.
- A change to `WORKSPACE`, `WORKSPACE.bazel`, `.bazelversion`, any
  `*.MODULE.bazel` file, any `MODULE.bazel` or `MODULE.bazel.lock` below the
  workspace root, or any `.bazelrc*` file, at any depth, widens the scope to
  `//...`. So does any edit to the root `MODULE.bazel` or `MODULE.bazel.lock`
  other than the `bazel_dep` version bump below. So does a file the root `.bazelrc` reaches
  through `import` or `try-import %workspace%/...`, including nested imports.
  Other `*.bazelrc` files do not widen. So does a file that `MODULE.bazel` or an
  included segment names by label in an extension tag, repo rule, or
  override, such as `requirements_lock.txt`, or a patch, and the included
  segment itself. A `crate.from_cargo` lockfile in an included segment counts
  as a lock input, as does the `lock_file` of a `maven.install`
  with literal arguments: only the users of artifacts whose entry changed in
  `maven_install.json` are selected. A `go.mod` named by `go_deps.from_file` and
  its `go.sum` do not widen: dx diffs their requirements and checksums and
  selects the users of the changed modules' Gazelle repositories, widening only
  for other `go.mod` edits (`replace`, `exclude`, `go`, `toolchain`). When dx
  cannot read every value in `MODULE.bazel`, any
  `pnpm-lock.yaml`, `package-lock.json`, `go.mod`, `go.sum`, `go.work`,
  `go.work.sum`, `maven_install.json`, or `requirements_lock.txt`, at any
  depth, widens to `//...`. A deleted, removed, or renamed-from file
  selects the targets that still name it. If no target names it, it widens to
  its package (for example `//pkg:all`) plus that package's reverse
  dependencies. A `.bzl` file follows the `.bzl` rule below. An unowned file widens to the package
  tree around it, such as `//pkg/...`. A file outside any package widens to
  `//...`.
- A changed `BUILD` or `BUILD.bazel` file selects every target in its package,
  plus its reverse dependencies. An added `BUILD` file widens to `//...`; a
  deleted one, or a deleted file outside any package, widens to `//...`.
- A change to a `Cargo.lock` or crate_universe `lockfile` named by a
  `crate.from_cargo` call in `MODULE.bazel` selects the targets that use the
  changed crates, plus the crates that depend on them, and their reverse
  dependencies. For a crate_universe lockfile this includes users of renamed,
  binary, and `extra_aliased_targets` hub aliases. Changes to `direct_deps`,
  `direct_dev_deps`, `workspace_members`, and `binary_crates` narrow. Any other
  `*.lock` file, an added or deleted lock, a lock dx cannot read, or a
  crate_universe lockfile change in another key outside `crates` (such as
  `conditions`) widens to `//...`. A changed crate_universe manifest such as `Cargo.toml` does
  not widen; it selects its owners like any other file. A patch or
  `additive_build_file` named in a `crate.annotation` tag widens to `//...`.
- A change to the `pnpm_lock` named by an `npm.npm_translate_lock` call in
  `MODULE.bazel` selects the targets that use the npm packages whose name or
  version changed (the `@npm//name` targets and the per-version spoke repos),
  plus their reverse dependencies. A change to the lock's top-level settings
  (such as `patchedDependencies` or `overrides`), or a lock dx cannot read,
  widens to `//...`.
- A root `MODULE.bazel` change that only edits `bazel_dep` version strings,
  with the matching `MODULE.bazel.lock` change, selects the targets in the
  repos of the changed modules and of every module that depends on them, plus
  their reverse dependencies. Repos that root module extensions create are
  selected too. Packages whose BUILD files load from those repos, directly or
  through `.bzl` files, are selected with their reverse dependencies. dx reads
  the module files from the Bazel repository cache without starting Bazel. It
  finds the cache from `--repository_cache` in `.bazelrc`, from
  `--output_user_root`, and from Bazel's default locations, and only trusts a
  file whose sha256 matches the lock. CI that restores the repository cache
  keeps the narrow selection. It widens to `//...` with one of these reasons:
  `workspace input MODULE.bazel` (another edit, no base commit, or a file
  added or missing), `workspace input MODULE.bazel.lock` (the lock is
  missing or does not list the bumped version, for example when it was not
  updated),
  `cannot read MODULE.bazel.lock: <detail>`,
  `MODULE.bazel.lock changed <key>` (a lock entry outside the changed
  modules), `MODULE.bazel has <override> for <module>` (`git_override`,
  `archive_override`, `local_path_override`, or an override with `patches`
  or `patch_cmds`), `no cached MODULE.bazel for <module>@<version>`,
  `cannot read MODULE.bazel of <module>@<version>: <detail>`,
  `changed module <module> registers toolchains` (including `bazel_tools`
  when a module it uses changes), `MODULE.bazel registers <label>`,
  `changed modules in MODULE.bazel reach toolchain <label>`,
  `module extension input loads @<repo>`, `aspect loads @<repo>`,
  `toolchain input loads @<repo>`, `bazelrc names @<repo>`, or
  `cannot read load graph: <detail>`. A loading package is listed with the
  reason `loads @<repo>`.
- A changed `.bzl` or `.scl` file selects the packages whose BUILD files load
  it, directly or through other `.bzl` files, plus their reverse dependencies.
  dx reads the BUILD and `.bzl` files to find them. Targets that own the
  file, such as a `bzl_library`, are selected too. It widens to
  `//...` when the file is used by a `MODULE.bazel` extension or repo rule,
  by an aspect named in `.bazelrc` or one dx runs, or by the BUILD file of a
  toolchain or platform package. Those packages are the ones named by
  `register_toolchains` or `register_execution_platforms` in `MODULE.bazel`,
  or by `--extra_toolchains`, `--extra_execution_platforms`, `--platforms`,
  or `--host_platform` in `.bazelrc`. The same applies to the rule
  implementation behind a `toolchain(toolchain = ...)` target in those
  packages: the BUILD file of a workspace label, or, for an `@repo` label,
  the loaded `.bzl` files that name the repo and the `.bzl` files they
  reference through `Label(...)`. It also widens to `//...` when a BUILD
  or `.bzl` file does not parse. A `.bzl` or `.scl` file nothing loads selects nothing.
  A deleted `.bzl` or `.scl` file selects the packages that still load it, or nothing.
- dx prints each widening and its reason on stderr:
  `dx: affected: 12 targets from 3 changed files since 1a2b3c4d5e6f; widened to //... (workspace input MODULE.bazel)`
  or
  `dx: affected: 4 targets from 1 changed files since 1a2b3c4d5e6f; widened to //app:all (loads tools/defs.bzl)`
  or
  `dx: affected: 4 targets from 1 changed files since 1a2b3c4d5e6f; widened to //app:all (build file app/BUILD.bazel)`.
  The line lists `//...` first and names at most three scopes, then
  `and N more`, for example
  `dx: affected: 0 targets from 1 changed files since 1a2b3c4d5e6f; widened to //p0:all (loads q/f.bzl), //p1:all (loads q/f.bzl), //p2:all (loads q/f.bzl) and 477 more`.
  `--output=json` lists every widened scope.
- Before the queries run, dx prints
  `dx: affected: resolving changes since 1a2b3c4d5e6f` on stderr, then the
  summary. `--quiet`, `--output=diff`, and `--output=json` print neither. An
  empty selection prints no summary.
- No changes, or no affected targets, prints `no affected targets` on stdout
  and exits `0`. `dx security` is the exception: when any file changed, it
  runs the secrets scan. With `--output=json` it emits `command_started` (with
  `affected`) and `command_finished`; `mode` is `check` for `dx check`, and for `dx fix`,
  `lint`, `typecheck` and `format` unless `--apply` is given; `dx build`,
  `test`, `coverage`, `security` and `license` report `default`.
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
  A `bazel_dep` bump adds one entry with the changed modules:
  `{"scope": "rdeps(//..., @leaf, @user)", "reason": "changed modules in MODULE.bazel", "repos": ["@leaf", "@user"]}`.
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
  `dx test`, `dx coverage`, and `dx check` then run one
  `tests(set(...))` query to find the affected tests. A change that widens
  to `//...` skips the tests query.
- `dx security --affected` runs the secrets scan whenever a file changed.
  When no dependency set owns a reached target, it checks no set and prints
  `Running audit security for no dependency sets`. With no changes it prints
  `no affected targets`.

```sh
bazel run @rules_dx//:dx -- test --affected
bazel run @rules_dx//:dx -- check --affected --output=json
```

