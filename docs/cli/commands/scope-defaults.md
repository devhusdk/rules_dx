# Scope defaults

Use this page to see which targets a dx command runs on when you name none, and how to narrow it.

A scope tells a command what to work on. It is a Bazel label such as `//cli:dx`, a pattern such as `//cli/...` for every target under `cli/`, a file or directory, or one of the `--here` and `--affected` flags. A target is one buildable unit, such as a library, a binary or a test.

```sh
dx lint --here
dx build //cli/...
dx test --affected
dx check --affected --output=json
```

## Defaults by command

| Command | No scope means |
| --- | --- |
| `build`, `test`, `coverage`, `fetch`, `bench`, `fuzz`, `lint`, `typecheck`, `format`, `check`, `security`, `license` | `//...`. Pass `--affected` for only what your changes reach. |
| `generate` | The whole repo. |
| `docs` | The docs sites, shards, and API trees under `//...`. |
| `update` | Every set. Or pass `set`, `set:package`, a label, or a path. `<set:package> <version>` takes one package and one version. |
| `run` | A runnable scope. With none, dx runs the only executable target in the repo, or lists up to 10 to pick from. |
| `deploy` | Exactly one main-workspace label. |
| `watch` | A watchable command plus its scopes. |
| `init` | An optional directory. |
| `completion` | Exactly one shell. |
| `clean`, `status`, `upgrade`, `worktree` | No scopes. |

## Scope shapes

- Labels and patterns: `//...`, `//pkg:target`. Only `dx run` accepts `@repo//...`.
- Files and directories: workspace-relative paths. A file resolves to the rules that use it. A directory becomes `//dir/...`. Paths with control characters are refused.
- `--here`: the current directory tree, as `//path/...` (`//...` at the root). Under `bazel run` it uses `BUILD_WORKING_DIRECTORY`. It does not combine with scopes. Accepted by `build`, `test`, `coverage`, `fetch`, `bench`, `fuzz`, `lint`, `typecheck`, `format`, `generate`, `check`, `security`, `license`, and `docs`.
- `--affected`: the targets your changes reach. `format` and the format phase of `check` use only the targets that read a changed file. `lint` and the lint phase of `check` add the targets that depend on those directly. `generate` and the generate phase of `check` use the directories of the changed files, without the packages below them. With `--apply`, `format`, `lint`, `typecheck`, and the other phases of `check` write only the changed files and the `BUILD` file of each one's package. A `MODULE.bazel.lock` change that only rewrites the `bzlTransitiveDigest` or `usagesDigest` of a module extension selects nothing. It does not combine with `--here`. With scopes, `build`, `test`, `coverage`, `fetch`, `lint`, `typecheck`, `format`, and `check` keep only the affected targets inside them, so `dx test --affected //svc/...` runs the affected tests under `//svc`; `generate`, `security`, and `license` reject scopes with it. Accepted by `build`, `test`, `coverage`, `fetch`, `lint`, `typecheck`, `format`, `generate`, `check`, `security`, and `license`. `docs` rejects it.

Targets that do not fit the target platform are skipped by `build`, `test`, `coverage`, `lint`, `typecheck`, `format`, and `check`. Skipped tests report `skipped`. Pass `-- --noskip_incompatible_explicit_targets` to fail on them. `dx run` fails on them.

## `--affected`

Changes are the diff from the merge-base with the base branch, plus staged, unstaged, and untracked files. The base branch is `--base <ref>` when given, else `base` under `[affected]` in [dx.toml](config.md), else the default branch (`origin/HEAD`, else `origin/main`, `main`, `origin/master`, `master`). A rename counts both paths. `--since <ref>` replaces the merge-base with that ref, for example `dx test --affected --since HEAD~1` for the last commit only. `--since` without `--affected` is a usage error, and a ref that names no commit exits `2`. In a subdirectory workspace, changes outside it are ignored.

Each changed file maps to its owning targets, then to their reverse dependencies in `//...`: the targets that depend on them. Targets tagged `manual` are left out.

- `format` and the format phase of `check` keep only the targets that read a changed file: the rules that own it, the rules that use a `filegroup` or `genrule` that owns it or a file such a rule generates, and the rules that take one of those, or an `alias` of one, as `crate`, `upstream`, `beam`, `beam_files_dir`, `dx_tfm_variants`, or `aspect_hints`, such as a Rust test of a library.
- `lint` and the lint phase of `check` keep those targets plus the targets that depend on them directly. They keep all reverse dependencies with `--lint-scope closure` or when a dependency version changes. A lint or format phase with no targets prints `no affected targets` and runs nothing. `typecheck` and the test phase of `check` keep all reverse dependencies.
- `test` and `coverage` keep only test targets.
- `security` and `license` keep only targets a dependency set owns. `security` still runs the secrets scan when any file changed.
- `check --affected` runs the checks, then the affected tests. `check --apply --affected` skips the tests.
- With no changes or no affected targets, dx prints `no affected targets` and exits `0`. With `--diff`, the line goes to stderr and stdout stays empty.

### What widens the scope

| Change | Selects |
| --- | --- |
| `BUILD` or `BUILD.bazel` | Every target in its package plus reverse dependencies. |
| An added or deleted `BUILD`, or a deleted file outside any package | `//...`. |
| A deleted file | The targets that still name it, else its package plus reverse dependencies. |
| An unowned file | The package tree around it, such as `//pkg/...`. |
| A file outside any package | `//...`. |
| A file that reaches a target a `.bazelrc` passes to a build setting, such as `.dx/config/typos/base.toml` through `--@rules_dx//config:workspace=//.dx:policy` | Its owning targets and the targets that read the setting, plus reverse dependencies. Lint and format aspects that read the setting do not widen. `//...` when a toolchain reads the setting. |
| A file that reaches a target another `.bazelrc` flag names, such as a toolchain or platform, or that an aspect attribute names | `//...`. |
| `.bzl` or `.scl` | The packages that load it, plus reverse dependencies. When a root module extension or `use_repo_rule` loads it, also the users of that extension's repos. `//...` when an aspect or a toolchain or platform package uses it, when the extension's repos are registered as toolchains, named in a `.bazelrc` or by an aspect attribute, or passed by label to another `MODULE.bazel` call, or when a BUILD or `.bzl` file does not parse. |
| `Cargo.lock` or crate_universe lockfile named by `crate.from_cargo` | The users of the changed crates. Another lock or an unreadable lock: `//...`. |
| Any lock or `go.mod` named by an extension tag | `//...` when another module in the graph also uses that extension (such as `rules_go` with `go_deps`), when a module's patch makes an edit listed under [Patches](#patches), when a module has a non-registry override, or when the extension's repos are passed by label to another `MODULE.bazel` call. Most real extension edits fall here. |
| `pnpm_lock` named by `npm.npm_translate_lock` | The users of the changed packages. A change to top-level settings: `//...`. |
| `maven_install.json` | The users of the changed artifacts. |
| `go.mod` and `go.sum` named by `go_deps.from_file` | Every user of a `go_deps` repo, because Go repos depend on each other. `replace`, `exclude`, `go`, or `toolchain` edits: `//...`. |
| Root `MODULE.bazel`, a `*.MODULE.bazel` it includes, or `MODULE.bazel.lock` | Changed or added `bazel_dep`s and changed overrides: the repos of those modules and their dependents. Changed extension tags, added `use_repo` names, and changed `use_repo_rule` calls: the repos of that extension or rule. Changed `go_deps` tags, including in `dev_dependency = True` usages, select every `go_deps` repo, because Go repos depend on each other. Changed extension entries in `MODULE.bazel.lock`: the repos of that extension. Each plus reverse dependencies. `//...` when a module registers toolchains from the changed extension's repos (such as `go_sdk` or `python.toolchain`), when another module also uses the changed extension, when a module's patch makes an edit listed under [Patches](#patches), when the extension's module has no `MODULE.bazel` in the registry lock, or when a changed repo is passed by label to another call, `inject_repo`, or `override_repo`. Comment and layout edits select nothing. |
| A file that an extension tag, `use_repo_rule` call, or override in `MODULE.bazel` names by label (such as `requirements_lock.txt` or a patch) | The repos of that extension, rule, or module, plus reverse dependencies. |
| Any other edit to the root `MODULE.bazel`, such as `register_toolchains`, a removed `bazel_dep` or `use_repo` name, or a call dx cannot read | `//...`. |
| A `.bazelrc*` file or a file the root `.bazelrc` imports, when the edit only changes comments, layout, or flags that do not change outputs (such as `--jobs`, `--disk_cache`, `--remote_cache`, `--color`, or `--show_progress`) | The targets that use the file as an input. |
| `WORKSPACE`, `WORKSPACE.bazel`, `.bazelversion`, a `*.MODULE.bazel` the root does not include, a nested `MODULE.bazel` or `MODULE.bazel.lock`, any other `.bazelrc*` edit, or a file `MODULE.bazel` names by label for an extension dx cannot resolve | `//...`. |
| A `local_path_override` path outside the workspace | `//...`. |
| Any `pnpm-lock.yaml`, `package-lock.json`, `go.mod`, `go.sum`, `go.work`, `go.work.sum`, `maven_install.json`, or `requirements_lock.txt` when dx cannot read every `MODULE.bazel` value | `//...`. |

A module change also widens to `//...` when the lock does not list the version, an override hides the `MODULE.bazel` of a changed module or patches it as listed under [Patches](#patches), a changed module registers toolchains or uses an extension outside its deps, a root `register_toolchains` or `register_execution_platforms` call names a changed repo, or dx cannot read a cached module file. CI that restores the repository cache keeps the narrow selection.

### Patches

dx reads each patch that a `single_version_override` or `multiple_version_override` applies. It looks at the lines a patch adds or removes in a `MODULE.bazel` file. When such a line is inside a call that spans several lines, dx also reads the first line of that call.

- A changed line with `bazel_dep` or `_override`: `//...`.
- A changed line with `register_`, `use_extension`, `use_repo_rule`, or `include`: `//...` when the patched module changed, or when the change touches an extension that a module other than the root owns.
- A changed line inside a call whose first line the patch does not show: `//...`.
- `patch_cmds`, or a patch dx cannot read: `//...`.

Other patch edits do not widen.

### Output

dx prints `dx: affected: resolving changes since <sha>` and then a summary on stderr. It names each widening and its reason, for example `dx: affected: 4 targets from 1 changed files since 1a2b3c4d5e6f; widened to //app:all (loads tools/defs.bzl)`. `--quiet`, `--diff`, and `--output=json` print neither.

`--output=json` adds `affected` to `command_started` with `base`, `changed_files`, `targets`, and `widened` (a list of `scope` and `reason`). `check --affected` adds `tests`. With `--coverage`, `tests` lists the tests of the `test` phase and `coverage_tests` the tests of the `coverage` phase.

### Errors

All exit `2`.

- Not in a git checkout: `<dir> is not inside a git work tree`.
- No default branch: `cannot find the default branch: expected origin/HEAD, main, or master`.
- Shallow checkout: run `git fetch --unshallow` or pass explicit scopes.
- A failed `bazel query`: `query_failed`, or `query_launch_failed` when it cannot start.
- `--affected` with `--here`, or with scopes on `generate`, `security`, `license`, or `deploy`.
- `--base` with `--since`, or without `--affected`.
