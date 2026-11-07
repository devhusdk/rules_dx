# Bazel for dx users

The Bazel terms and behaviours you meet in your first hour with `dx`. Each
section says what you will see and what to do.

## Workspace, module, package, target, label

- **Module**: the repository root, marked by `MODULE.bazel`.
- **Package**: a directory that contains a `BUILD.bazel` file.
- **Target**: a named thing in a package, such as a library or a test.
- **Label**: the full name of a target: `//cli/worktree:worktree_test`.
  `//` is the module root, the part before `:` is the package, the part after
  is the target name. Inside a package, `:name` is enough.
- `//cli/worktree/...` means every target in that package and below.
  `//...` means every target in the repo. Prefer a narrow pattern, since
  `//...` builds and tests everything.

## BUILD files are generated

Run `dx generate --apply` after you add, move, or delete source files or
change imports. It writes the `BUILD.bazel` files. See
[generate](cli/commands/generate.md).

- Safe to hand-edit: attributes dx does not own, such as `data`, `env`,
  `tags`, `size`, and `visibility`. Keep them next to the target; generate
  keeps them.
- Do not hand-edit: source lists and dependency lists that generate writes.
  The next `dx generate --apply` overwrites your change.

## Visibility

Targets are private to their package unless `visibility` says otherwise.
The error looks like `target '//a:lib' is not visible from target '//b:app'`.
Fix it on the target being used:

```starlark
visibility = ["//b:__pkg__"],   # one package
visibility = ["//b:__subpackages__"],   # a package and everything below
visibility = ["//visibility:public"],   # everyone
```

## Tests run in a sandbox

A test sees only what it declares, in a directory of its own.

- Files: list them in `data`. They appear under the test's runfiles
  directory. Read them through the runfiles library or a path relative to the
  test's working directory.
- `../fixtures/x.json` fails because the sandbox does not contain your
  source tree. Put `x.json` in `data` (or in a target listed in `data`) and
  open it by its path inside the runfiles.
- Environment variables are not passed through. Set fixed ones with `env`
  on the target, or pass one for a single run with
  `--test_env=NAME` (value from your shell) or `--test_env=NAME=value`.
- There is no network by default. A test that needs it must say so, for
  example with the `requires-network` tag.

## Test caching

`(cached) PASSED` means the inputs of the test did not change since the last
pass, so Bazel did not run it again. This is correct and not a stale result.

To force a rerun, use `--nocache_test_results`, or `--runs_per_test=1`. To
rerun after changing only an env var, pass it with `--test_env`. Changing a
file listed in `data` reruns the test by itself.

## dx run and bazel run

`dx run` and `bazel run` start the program from its runfiles directory, not
from your current directory. Relative paths you pass in the program resolve
there. To find where you started, read `BUILD_WORKING_DIRECTORY`:

```sh
cd "$BUILD_WORKING_DIRECTORY"
```

## Outputs and logs

- Build outputs: `bazel-bin/<package>/<target>`.
- Test logs: `bazel-testlogs/<package>/<test>/test.log`. Failures print the
  path at the end of the run.
- The `bazel-*` entries in the repo root are symlinks. Add them to
  `.gitignore`; do not edit them.

## The Bazel server and disk use

Bazel keeps one background server per repo. A second Bazel command in the
same repo waits with `Another command is running`; wait for it or stop the
first. The server holds memory between commands. It exits after idle time, or
stop it with `bazel shutdown`.

Build outputs and caches grow over time. `dx clean --bazel` frees disk; see
[check, clean](cli/commands/check.md).

## Generated targets dx adds

A dx macro adds targets next to yours. They show up in `bazel query`, error
messages, and `--test_filter`. Do not reuse these names in the same package.

- Every dx wrapper: `<name>_upstream`. Private.
- `typescript_project`: `tsconfig_<name>.json` (generated file).
- TypeScript tests: `<name>_ts`. Private.
- Kotlin with `kotlinc_opts`: `<name>_kotlinc_opts`. Private.
