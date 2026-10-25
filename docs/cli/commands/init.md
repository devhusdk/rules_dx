# `dx init`

```sh
bazel run @rules_dx//:dx -- init --apply
```

## `dx init`

```text
dx init [--apply] [module-name]
```

Adopts `dx` into an existing tree. Use `dx new` to start a new project. Checks by default; `--apply` scaffolds. Takes an optional module name, which
defaults to `my_project`. The module name starts with `[a-z0-9]` and uses
`[a-z0-9._-]` only. An invalid module name fails before any file is written.
Never overwrites existing files. It writes `.dx/version`, `.gitignore`, `.envrc`, `.devcontainer/devcontainer.json`,
`.vscode/settings.json`, `.vscode/extensions.json`,
`.github/workflows/ci.yml`, `MODULE.bazel.snippet`, the launcher
(`tools/dx` and `tools/dx.cmd`), and `.claude/settings.json`
with the `dx worktree` hooks. The hooks run `tools/dx`, which also runs in a Git Bash or MSYS2 shell on Windows. It also writes a marked block to `AGENTS.md`,
creating the file when it is missing (see below). If `.claude/settings.json`
already exists, it is left alone and the missing hook entries are printed to
stderr. The CI caller pins the
`rules_dx` reusable workflow at a reviewed commit and passes
`rules_dx_version` matching the snippet. The `.gitignore` lists `target/`, `/bazel-*`, `node_modules/`,
`__pycache__/`, `*.pyc`, `.gradle/` and `/build/`. Editor settings point at the managed
interpreter and tools with host-native names.
Flags: `--apply`.
Scopes: optional single module name.

Output: text only.

Exit codes: `0` success, `2` usage errors including extra positionals or an
invalid module name, `1` scaffolding failed.

## Agent commands block

`dx init --apply` writes this block between `<!-- dx:begin -->` and
`<!-- dx:end -->` markers in `AGENTS.md`:

```text
<!-- dx:begin -->
- Start: `tools/dx worktree add <branch>`. Finish: `tools/dx worktree release`.
- Fix first: `tools/dx fix --apply --affected`.
- Then verify: `tools/dx check --affected`.
- Read the `--output=json` result.
<!-- dx:end -->
```

An existing file gets the block appended. A block that is out of date is
replaced. Text outside the markers is never touched, and CRLF files keep CRLF.
A begin marker without an end marker is an error and the file is left as is.
`dx status` reports the `agents` check as `warn` when the block is missing or
out of date, or a hook is missing from `.claude/settings.json`.

## Launcher

`tools/dx` (sh) and `tools/dx.cmd` run dx without Bazel. They read `.dx/version`,
download the matching standalone binary and its `.sha256` file from the GitHub
release into `~/.cache/dx/bin/<version>/` (`%USERPROFILE%\.cache\dx\bin\<version>\`
on Windows), check the binary against that sha256, and run it. A cached binary
is only run. No Bazel server starts.

```text
tools/dx version
tools/dx check //...
```

Binaries exist for Linux x86_64 and arm64, macOS arm64, and Windows x86_64 and
arm64. The launcher exits `1` and prints the `bazel run @rules_dx//:dx -- ...`
command to use instead when the download fails, the platform has no binary, or
`.dx/version` differs from the launcher's version. A binary that does not match
its sha256 is deleted and never run.

`dx init --apply` writes the launcher absent-only, pinned to the running dx
version.

In a `rules_dx` source checkout the launcher skips the download and the pin. It
runs `bazel build //:dx` when `bazel-bin/cli/cli/dx` is missing or any input of
`//cli/cli:dx` changed since the last build, then runs that `dx`. Inputs are the
source and BUILD files Bazel reports for `//cli/cli:dx`, plus `.bazelrc`,
`.bazelversion`, `user.bazelrc`, `tools/bazelrc/preset.bazelrc`, `MODULE.bazel`,
`MODULE.bazel.lock` and `cargo-bazel-lock.json`. The list and the build time
live in `.dx/launcher/`. An unchanged tree runs `dx` without starting Bazel.
Under `CI=true` and inside Bazel tests, the build adds `--nohome_rc --nosystem_rc`.
`dx worktree hook` commands run the last built `dx` without starting Bazel, and
build only when `bazel-bin/cli/cli/dx` is missing. The next other command
rebuilds.

Run from inside another `rules_dx` checkout, such as a `dx worktree` slot, the
launcher runs that checkout's `tools/dx`, which builds and runs that checkout's
`dx`. `dx worktree hook` commands still run the launcher's own `dx` when it is
built.

## Git hooks

dx does not install or manage Git hooks. For a commit or push hook, write a
plain hook or use a hook tool, and have it run:

```sh
bazel run @rules_dx//:dx -- check //...
```

Enforce the same check in CI. See [GitHub CI](../../github-ci.md).
