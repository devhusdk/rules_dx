# Scope defaults

```sh
dx lint --here
dx build //cli/...
dx test --affected
dx check --affected --output=json
```

## Defaults by command

| Command | No scope means |
| --- | --- |
| `build`, `test`, `coverage`, `bench`, `fuzz`, `lint`, `typecheck`, `format`, `check`, `security`, `license` | `//...`. Pass `--affected` for only what your changes reach. |
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
- `--here`: the current directory tree, as `//path/...` (`//...` at the root). Under `bazel run` it uses `BUILD_WORKING_DIRECTORY`. It does not combine with scopes. Accepted by `build`, `test`, `coverage`, `bench`, `fuzz`, `lint`, `typecheck`, `format`, `generate`, `check`, `security`, `license`, and `docs`.
- `--affected`: the targets your changes reach. It does not combine with scopes or `--here`. Accepted by `build`, `test`, `coverage`, `lint`, `typecheck`, `format`, `check`, `security`, and `license`. `generate` and `docs` reject it.

Targets that do not fit the target platform are skipped by `build`, `test`, `coverage`, `lint`, `typecheck`, `format`, and `check`. Skipped tests report `skipped`. Pass `-- --noskip_incompatible_explicit_targets` to fail on them. `dx run` fails on them.

## `--affected`

Changes are the diff from the merge-base with the default branch (`origin/HEAD`, else `origin/main`, `main`, `origin/master`, `master`), plus staged, unstaged, and untracked files. A rename counts both paths. `--since <ref>` replaces the merge-base with that ref, for example `dx test --affected --since HEAD~1` for the last commit only. `--since` without `--affected` is a usage error, and a ref that names no commit exits `2`. In a subdirectory workspace, changes outside it are ignored.

Each changed file maps to its owning targets, then to their reverse dependencies in `//...`. Targets tagged `manual` are left out.

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
| `.bzl` or `.scl` | The packages that load it, plus reverse dependencies. `//...` when `MODULE.bazel`, an aspect, or a toolchain or platform package uses it, or a BUILD or `.bzl` file does not parse. |
| `Cargo.lock` or crate_universe lockfile named by `crate.from_cargo` | The users of the changed crates. Another lock, an unreadable lock, or a patch named in `crate.annotation`: `//...`. |
| `pnpm_lock` named by `npm.npm_translate_lock` | The users of the changed packages. A change to top-level settings: `//...`. |
| `maven_install.json` | The users of the changed artifacts. |
| `go.mod` and `go.sum` named by `go_deps.from_file` | The users of the changed modules. `replace`, `exclude`, `go`, or `toolchain` edits: `//...`. |
| Root `MODULE.bazel` edit that only changes `bazel_dep` versions, with its `MODULE.bazel.lock` | The repos of the changed modules and their dependents, plus reverse dependencies. |
| Any other edit to the root `MODULE.bazel` or `MODULE.bazel.lock` | `//...`. |
| `WORKSPACE`, `WORKSPACE.bazel`, `.bazelversion`, `*.MODULE.bazel`, a nested `MODULE.bazel` or `MODULE.bazel.lock`, any `.bazelrc*`, a file the root `.bazelrc` imports, or a file `MODULE.bazel` names by label (such as `requirements_lock.txt` or a patch) | `//...`. |
| A `local_path_override` path outside the workspace | `//...`. |
| Any `pnpm-lock.yaml`, `package-lock.json`, `go.mod`, `go.sum`, `go.work`, `go.work.sum`, `maven_install.json`, or `requirements_lock.txt` when dx cannot read every `MODULE.bazel` value | `//...`. |

A `bazel_dep` update also widens to `//...` when the lock does not list the version, a module has an override, a changed module registers toolchains, or dx cannot read a cached module file. CI that restores the repository cache keeps the narrow selection.

### Output

dx prints `dx: affected: resolving changes since <sha>` and then a summary on stderr. It names each widening and its reason, for example `dx: affected: 4 targets from 1 changed files since 1a2b3c4d5e6f; widened to //app:all (loads tools/defs.bzl)`. `--quiet`, `--diff`, and `--output=json` print neither.

`--output=json` adds `affected` to `command_started` with `base`, `changed_files`, `targets`, and `widened` (a list of `scope` and `reason`). `check --affected` adds `tests`.

### Errors

All exit `2`.

- No default branch: `cannot find the default branch: expected origin/HEAD, main, or master`.
- Shallow checkout: run `git fetch --unshallow` or pass explicit scopes.
- A failed `bazel query`: `query_failed`, or `query_launch_failed` when it cannot start.
- `--affected` with scopes or `--here`.
