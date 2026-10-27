# `dx init`

```sh
bazel run @rules_dx//:dx -- init
bazel run @rules_dx//:dx -- init --apply
bazel run @rules_dx//:dx -- init --apply my_module
```

```text
tools/dx init
```

## `dx init`

```text
dx init [--apply] [module-name]
```

Adopts `dx` into an existing tree. Use `dx new` to start a new project. Checks by default; `--apply` scaffolds. Takes an optional module name, which
defaults to `my_project`. The module name starts with `[a-z0-9]` and uses
`[a-z0-9._-]` only. An invalid module name fails before any file is written.
Writes each file only when it is missing. The exceptions are the marked
block in `AGENTS.md` and stale launchers (see below). It writes `.dx/version`, `.gitignore`, `.bazelignore`, `.envrc`, `.devcontainer/devcontainer.json`,
`.vscode/settings.json`, `.vscode/extensions.json`,
`.github/workflows/ci.yml`, `MODULE.bazel.snippet`, the launcher
(`tools/dx` and `tools/dx.cmd`), and the `dx worktree` hooks for each agent:

| Agent | Hooks file | Hooks |
| --- | --- | --- |
| Claude Code | `.claude/settings.json` | all four |
| Cursor | `.claude/settings.json`, through Third-Party Imports | all four |
| Codex | `.codex/hooks.json` | `guard`, after you trust it with `/hooks` |
| Gemini CLI | `.gemini/settings.json` | `guard`, in a trusted folder |

See [Hooks](worktree.md#hooks). The hooks run `tools/dx`, which also runs in a Git Bash or MSYS2 shell on Windows. It also writes a marked block to `AGENTS.md`,
creating the file when it is missing (see below). It writes `CLAUDE.md` with an
`@AGENTS.md` import when the import is missing, so Claude Code reads the block.
The new `.claude/settings.json` also carries a `permissions.allow` list for the
read-only commands `tools/dx check *`, `test *`, `build *`, `list-tests *`,
`status *` and `version *`. If a hooks file
already exists, it is left alone and the missing hook and allow entries are printed to
stderr. The CI caller pins the
`rules_dx` reusable workflow at a reviewed commit and passes
`rules_dx_version` matching the snippet. When the tree has `.cu` or `.cuh` files, the snippet also carries the pinned CUDA toolkit and host-compiler stanza and the opt-in `cuda-eula` license line (see `dx new cuda`). The `.gitignore` lists `target/`, `/bazel-*`, `node_modules/`,
`__pycache__/`, `*.pyc`, `.gradle/`, `/build/`, `/.dx/ide/` and
`/.vscode/.rules_rust_analyzer/`. The `.bazelignore` lists `.dx`, so Bazel skips
the IDE output bases. When a `.bazelignore` already exists it is left alone, and
`dx init --apply` prints `dx: .bazelignore lacks .dx; add it so Bazel skips the IDE output base`
to stderr if it does not list `.dx`. Editor settings point at the managed
interpreter and tools with host-native names. For Rust they set up
rust-analyzer with the `rules_rust` discover and flycheck targets. Both run
with `bazel run` on separate output bases under `.dx/ide/`, and the file
watcher and search exclude `bazel-*` and `.dx/ide/`. They configure language servers, not debuggers. See
[Supported Surface](README.md#supported-surface).
Flags: `--apply`.
Scopes: optional single module name.

Output: text only. Check mode looks for `.dx/version`,
`.devcontainer/devcontainer.json`, `.gitignore`, `.bazelignore`, `.envrc`,
`.vscode/settings.json`, `.vscode/extensions.json`,
`.github/workflows/ci.yml`, `MODULE.bazel.snippet`, `tools/dx`,
`tools/dx.cmd`, `.claude/settings.json`, `AGENTS.md` and `CLAUDE.md`.
It also flags a dx-written launcher whose version differs from `.dx/version`,
an `AGENTS.md` without the current dx block, a hooks file that lacks dx hooks
(`.codex/hooks.json` and `.gemini/settings.json` only when they exist), a
`CLAUDE.md` without the `@AGENTS.md` import, and an empty or non-semver
`.dx/version`.
When nothing is missing or stale and the pin is valid, it prints
`init current: <module>` to stdout. Otherwise it prints
`dx: init_failed: init drift: missing <paths>; stale <paths>; <path> lacks hooks: <hooks>; CLAUDE.md lacks @AGENTS.md; invalid .dx/version ("<pin>")`
to stderr (only the parts that apply), followed by the `dx init <module> --apply`
command to run. Apply does not edit an existing hooks file; it prints the hooks
to add. An invalid pin is not fixed by apply: write the module version to
`.dx/version`, which the message names.
Apply checks every destination before the first write. A path whose parent is
a file, such as a `.vscode` file, an unreadable hooks file, or an `AGENTS.md`
with a begin marker and no end marker fails with `init_failed` and writes
nothing. Apply then writes every file and prints afterwards. It prints `wrote <path>` to stdout for each file it creates or refreshes,
`wrote AGENTS.md` when the block changes, and `wrote CLAUDE.md` when the import is added. For each scaffold or launcher file
that already exists and is kept, it prints `dx: <path> (absent-only, left untouched)` to
stderr. `--quiet` hides the stdout lines. The stderr lines still print. When a
write fails, apply still lists the files it already wrote, then the
`init_failed` error. Closing stdout early does not stop the writes.

Exit codes: `0` check passed or apply succeeded, `2` usage error, including
extra positionals or an invalid module name, `1` check found drift or apply
failed, `141` apply wrote every file but stdout closed early.

## rules_dx overrides

`dx init --apply` and `dx new --apply` write the root-only
`single_version_override`s `rules_dx` needs to `third_party/rules_dx/`:
`rules_dx.MODULE.bazel`, one `<module>.<file>.patch` per patch, `BUILD.bazel`
and `.gitattributes`. These files are always rewritten to match the running dx.
When `MODULE.bazel` has `bazel_dep(name = "rules_dx")`, apply adds
`include("//third_party/rules_dx:rules_dx.MODULE.bazel")` after it and prints
`wrote MODULE.bazel`. `MODULE.bazel.snippet` carries the same line. When
`MODULE.bazel` already overrides one of the modules, the line is left out and
stderr prints `dx: MODULE.bazel (overrides <modules> itself; ...)`. Inside the
`rules_dx` repository nothing is written. See
[patches/README.md](../../../patches/README.md).

## Agent commands block

`dx init --apply` writes this block between `<!-- dx:begin -->` and
`<!-- dx:end -->` markers in `AGENTS.md`. `dx new --apply` writes the same
block in the new project:

```text
<!-- dx:begin -->
- Start: `tools/dx worktree add <branch>`. Finish: `tools/dx worktree release`.
- Fix first: `tools/dx fix --apply --affected`.
- Then verify: `tools/dx test --affected`.
- Read the `--output=json` result.
<!-- dx:end -->
```

An existing file gets the block appended. A block that is out of date is
replaced. Text outside the markers is never touched, and CRLF files keep CRLF.
A begin marker without an end marker is an error and the file is left as is.
`dx status` reports the `agents` check as `warn` when the block is missing or
out of date, `CLAUDE.md` does not import `AGENTS.md`, or a hook is missing from
`.claude/settings.json`, or from `.codex/hooks.json` or `.gemini/settings.json`
when they exist.

## Launcher

`tools/dx` (sh) and `tools/dx.cmd` run dx without Bazel. They read `.dx/version`,
download the matching standalone binary and its `.sha256` file from the GitHub
release into `<cache>/bin/<version>/`, check the binary against that sha256,
and run it. A cached binary is only run. No Bazel server starts.

`<cache>` is `DX_CACHE_DIR`, else `$XDG_CACHE_HOME/dx`, else `~/.cache/dx`
(`%USERPROFILE%\.cache\dx` on Windows). The [global shim](README.md#global-shim)
uses the same cache, so each runs binaries the other downloaded.

```text
tools/dx version
tools/dx check //...
```

Binaries exist for Linux x86_64 and arm64, macOS arm64, and Windows x86_64 and
arm64. The download launcher exits `1` and prints the `bazel run @rules_dx//:dx -- ...`
command to use instead when the download fails or the platform has no binary.
A binary that does not match its sha256 is deleted and never run.

When `.dx/version` differs from the launcher's version, the launcher exits `1`
and prints:

```text
dx: tools/dx is pinned to dx <v> but .dx/version is '<pin>'; refresh it: bazel run @rules_dx//:dx -- init --apply
```

`dx init --apply` writes a missing launcher and rewrites one whose `VERSION`
line differs from `.dx/version`. With no `.dx/version` it uses the running dx
version. dx treats a launcher as its own when it starts with the template's
first three lines (`#!/bin/sh`, `set -eu`, `VERSION="<v>"`, or `@echo off`,
`setlocal`, `set "VERSION=<v>"`). Any other launcher is yours and is never
touched. A dx launcher is rewritten when stale, so hand edits to it are lost.
When a launcher would change and `.dx/version` is not semver, init fails with
`init_failed` before writing any file. `dx rollback --apply` refreshes existing launchers the
same way. `dx new --apply` writes the same launcher in the new project.

## Git hooks

dx does not install or manage Git hooks. For a commit or push hook, write a
plain hook or use a hook tool, and have it run:

```sh
bazel run @rules_dx//:dx -- check //...
```

Enforce the same check in CI. See [GitHub CI](../../github-ci.md).
