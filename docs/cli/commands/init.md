# `dx init`

Empty directory, with Bazel. Create `MODULE.bazel` with
`bazel_dep(name = "rules_dx", version = "<release>")` and a `.bazelversion`, then:

```sh
bazel run @rules_dx//:dx -- init --apply
bazel run @rules_dx//:dx -- test //...
```

Empty directory, with the global shim (coming):

```sh
DX_VERSION=<release> dx init --apply
dx test //...
```

Existing tree:

```sh
bazel run @rules_dx//:dx -- init
bazel run @rules_dx//:dx -- init --apply
bazel run @rules_dx//:dx -- init --apply my-app
```

```text
tools/dx init
```

## `dx init`

```text
dx init [--apply] [dir]
```

Starts a new workspace or adopts `dx` into an existing tree. Checks by
default; `--apply` scaffolds. Takes an optional directory, relative to the
workspace and defaulting to the current one. `--apply` creates the directory
when it is missing. A directory that is missing or holds only `.git` is fresh:
init also writes a `MODULE.bazel` that depends on `rules_dx` and a
`.bazelversion`. A tree with other files keeps its own `MODULE.bazel`.
Nothing language-specific is chosen up front; languages are wired when their
sources appear.

The module name is the directory name, lowercased, with every run of other
characters folded to `_`, or `my_project` for the current directory. An
absolute path, a `..` segment, or a name with no `[a-z0-9]` character fails
before any file is written.

It writes every path in the [fixed file set](#fixed-file-set). Re-running
`dx init --apply` is safe: it keeps your edits outside the rows owned by `dx`.

`MODULE.bazel.snippet` holds the `rules_dx` pin and one `include()` of the
rules_dx overrides. It carries no toolchain versions and no language
stanzas, and it is the same for every tree.

Init writes the `dx worktree` hooks for each agent:

| Agent | Hooks file | Hooks |
| --- | --- | --- |
| Claude Code | `.claude/settings.json` | all four |
| Cursor | `.claude/settings.json`, through Third-Party Imports | all four |
| Codex | `.codex/hooks.json` | `guard`, after you trust it with `/hooks` |
| Gemini CLI | `.gemini/settings.json` | `guard`, in a trusted folder |

See [Hooks](worktree.md#hooks). The hooks run `tools/dx`, which also runs in a Git Bash or MSYS2 shell on Windows. It also writes a marked block to `AGENTS.md`,
creating the file when it is missing (see below). It writes `CLAUDE.md` with an
`@AGENTS.md` import when the import is missing.
The new `.claude/settings.json` also carries a `permissions.allow` list for the
read-only commands `tools/dx check *`, `test *`, `build *`,
`status *` and `version *`. If a hooks file
already exists, the missing hook and allow entries are added in place and your
other entries stay.

The CI caller pins the
`rules_dx` reusable workflow at a reviewed commit and passes
`rules_dx_version` matching the snippet. The `.gitignore` lists `target/`, `/bazel-*`, `node_modules/`,
`__pycache__/`, `*.pyc`, `.gradle/`, `/build/`, `/.dx/ide/`, `/.dx/bazelrc/` and
`/.vscode/.rules_rust_analyzer/`. The `.bazelignore` lists `.dx`. When a `.bazelignore` already exists it is left alone, and
`dx init --apply` prints `dx: .bazelignore lacks .dx; add it so Bazel skips the IDE output base`
to stderr if it does not list `.dx`. The editor settings point at the managed
interpreter and tools with host-native names. For Rust they set up
rust-analyzer with the `rules_rust` discover and flycheck targets. Both run
with `bazel run` on separate output bases under `.dx/ide/`, and the file
watcher and search exclude `bazel-*` and `.dx/ide/`. They configure language servers, not debuggers. See
[Supported Surface](README.md#supported-surface).
Flags: `--apply`.
Scopes: optional single directory, created by `--apply` when missing.

Output: text only. Check mode looks for every file init writes.
It also flags a missing preset import. It flags a dx-written launcher that differs from the current launcher text
and an empty or non-semver `.dx/version`. It flags
missing `.claude/settings.json`, `AGENTS.md` and `CLAUDE.md`,
an `AGENTS.md` without the current dx block, a hooks file that lacks dx hooks
(`.codex/hooks.json` and `.gemini/settings.json` only when they exist), and a
`CLAUDE.md` without the `@AGENTS.md` import.
When nothing is missing or stale and the pin is valid, it prints
`init current: <module>` to stdout. Otherwise it prints
`dx: init_failed: init drift: missing <paths>; stale <paths>; <path> lacks hooks: <hooks>; CLAUDE.md lacks @AGENTS.md; invalid .dx/version ("<pin>")`
to stderr (only the parts that apply), followed by the `dx init --apply` command to run
(`dx init <dir> --apply` when a directory was given). Apply adds the missing hooks to an existing hooks file. An invalid pin is not fixed by apply: write the module version to
`.dx/version`, which the message names.
Apply checks every destination before the first write. A path whose parent is
a file, such as a `.vscode` file, an unreadable hooks file, or an `AGENTS.md`
with a begin marker and no end marker fails with `init_failed` and writes
nothing. Apply then writes every file and prints afterwards. It prints `wrote <path>` to stdout for each file it creates or refreshes,
including `wrote AGENTS.md` when the block changes, and `wrote CLAUDE.md` when the import is added. For each scaffold or launcher file
that already exists and is kept, it prints `dx: <path> (absent-only, left untouched)` to
stderr. `--quiet` hides the stdout lines. The stderr lines still print. When a
write fails, apply still lists the files it already wrote, then the
`init_failed` error. Closing stdout early does not stop the writes.

Output: `--output text|json`. JSON covers the check only; see Init plan below.

Exit codes: `0` check passed or apply succeeded, `2` usage error, including
extra positionals, an unknown flag, or an invalid directory, `1` check found drift or apply
failed, `141` apply wrote every file but stdout closed early.

## Fixed file set

`dx init --apply` writes these paths. Owner `you` means you edit the file and
dx adds only the lines named here. Owner `dx` means dx rewrites the file to
match the running dx.

| Path | Owner | What `dx init --apply` updates |
| --- | --- | --- |
| `.dx/version` | dx | Written when missing. Line 1 is the version, then one sha256 line per release binary. `dx upgrade --apply` rewrites it. |
| `tools/dx`, `tools/dx.cmd` | dx | Written when missing. Rewritten when it differs from the current launcher text. A launcher dx did not write is kept. |
| `third_party/rules_dx/` | dx | Rewritten on every run: `rules_dx.MODULE.bazel`, the patches, `BUILD.bazel` and `.gitattributes`. |
| `.dx/bazelrc/correctness.bazelrc`, `performance.bazelrc`, `resources.bazelrc` | dx | Generated, not committed. Written when missing. Rewritten when they carry the dx header and differ. A file without the header is kept. |
| `.bazelrc` | you | Created when missing. Three `try-import %workspace%/.dx/bazelrc/<layer>.bazelrc` lines are added at the top when none exists. Other lines stay. |
| `MODULE.bazel` | you | Written in a fresh directory. Otherwise the `include` of the rules_dx overrides is added after `bazel_dep(name = "rules_dx")`, unless the file overrides a rules_dx module. |
| `.bazelversion` | you | Written in a fresh directory only. |
| `MODULE.bazel.snippet` | you | Written when missing. Never rewritten. |
| `.gitignore` | you | Written when missing. An existing file gets `/.dx/bazelrc/` appended when no line covers it. |
| `.bazelignore`, `.envrc`, `.devcontainer/devcontainer.json`, `.vscode/extensions.json` | you | Written when missing. Never rewritten. |
| `.vscode/settings.json` | you, and dx for its own keys | Written when missing. Only the keys dx sets are updated; comments, other keys and order stay. |
| `.github/workflows/ci.yml` | you, and dx for the pin | Written when missing. Only the `uses:` pin of the rules_dx workflow and `rules_dx_version` are updated. A file without `rules_dx_version` is left untouched. |
| `AGENTS.md` | you, and dx for the marked block | Created when missing. Only the text between `<!-- dx:begin -->` and `<!-- dx:end -->` is rewritten. |
| `CLAUDE.md` | you | Created when missing. `@AGENTS.md` is added when the import is missing. |
| `.claude/settings.json` | you, and dx for its hooks and allow entries | Written when missing. When it exists, the missing hooks and `permissions.allow` entries are added in place. |
| `.codex/hooks.json`, `.gemini/settings.json` | you, and dx for the guard hook | Written when missing. When they exist, the guard hook is added if it is missing. |

Check reports a missing or stale path as drift. An old dx key in `.vscode/settings.json`, or an old pin or `rules_dx_version` in `.github/workflows/ci.yml`, is stale. Deleting `.codex/hooks.json`
or `.gemini/settings.json` stops check from reporting it, but `dx init --apply`
writes it again. To opt out of a preset layer, comment out its import (see
[Bazel preset](#bazel-preset)).

## Init plan

When the root has a `MODULE.bazel`, `dx init` (the check) also prints a read-only plan of
what adopting rules_dx would change, found from the declared files and `MODULE.bazel.lock`. It runs no Bazel
and writes nothing. Sections:

- `version raises`: a module that resolves below the version rules_dx needs (the highest of
  its own `bazel_dep` floors, the snippet init would write, and the version in
  `modules/consumer_graph.json`, see below). For a module dx patches, the
  target is the exact version dx pins, and a pin below yours is reported as lower. Modules
  you override yourself are skipped. The current version is the one in `MODULE.bazel.lock`
  (the highest `source.json` it records, so transitive raises show, with the lock line), or
  the declared `bazel_dep` version when the lock is missing, invalid, or stale (a declared
  dep it lacks or has at a lower version); a note then says the comparison is declared-only.
  Advisory.
- `override conflicts`: a `*_override` you own. One on a module dx patches
  (`single_version_override` of `rules_rust`, for example) is hard, because dx cannot add its
  patches; one on another rules_dx dependency is advisory.
- `toolchain conflicts`: toolchains you register that compete with registrations the snippet
  would add, measured against the merged file, so a repo that already has the snippet shows
  none. A Rust toolchain (your `rust.toolchain` tag, or a registered repo from your own
  `rules_rust` extension) or a `toolchains_llvm` / `cc_configure` registration is hard.
  Bazel prefers root-module registrations in listed order (first wins) and
  `--extra_toolchains` beats all of them, see
  <https://bazel.build/extending/toolchains>. A `python.toolchain` of yours is advisory:
  rules_dx registers Python only as a dependency module, so yours always wins. Registrations
  of repos dx cannot link to an ecosystem are never flagged.
- `bazelrc conflicts`: a line in the root `.bazelrc`, or a file it imports through
  `%workspace%/`, that exactly repeats a flag line the dx preset layers own. Advisory; dx
  does not import the layers while the root file repeats them, so drop the line.

The "after" versions include transitive ones. `modules/consumer_graph.json` lists every module
a fresh dx consumer resolves to. Bazel picks the highest version any module
requires (<https://bazel.build/external/module#version-selection>), so your graph plus dx
resolves to at least the higher of your version and the file's. This is an approximation of a
real resolution: a raise can be under-reported, never invented. dx does not run Bazel on a copy
of your workspace.

| Exit code | Meaning |
| --- | --- |
| `0` | no hard findings (raises and advisory findings are printed) |
| `1` | a hard override or toolchain conflict, or other init drift |
| `2` | usage error |

Each finding names the file and line, including `include()`d `.MODULE.bazel` files. Hard
findings add an `init plan: N hard conflicts (...)` clause to `init drift` and exit 1;
advisory findings, and an unavailable lock, never change the exit code.

With `--output=json`, the plan is one `init_plan` event per finding instead of text, with
`kind` (`version_raise`, `override_conflict`, `toolchain_conflict`, `bazelrc_conflict`),
`subject`, `before`, `after`, `path`, `file`, `line` (left out when unknown), `hint`, and `severity`
(`hard` or `advisory`). `file` is a deprecated alias of `path` and goes away in the next major. Plan notes, such as the lock being unavailable, are `notice` events
with code `init_plan_partial`. `--apply` refuses `--output=json`.

## rules_dx overrides

`dx init --apply` writes the root-only
`single_version_override`s `rules_dx` needs to `third_party/rules_dx/`:
`rules_dx.MODULE.bazel`, one `<module>.<file>.patch` per patch, `BUILD.bazel`
and `.gitattributes`. These files are always rewritten to match the running dx.
When `MODULE.bazel` has `bazel_dep(name = "rules_dx")`, apply adds
`include("//third_party/rules_dx:rules_dx.MODULE.bazel")` after it and prints
`wrote MODULE.bazel`. `MODULE.bazel.snippet` carries the same line. When
`MODULE.bazel` already overrides one of the modules, the line is left out and
stderr prints `dx: MODULE.bazel (overrides <modules> itself; ...)`. Inside the
`rules_dx` repository nothing is written. See
[patches/README.md](https://github.com/devhusdk/rules_dx/blob/main/patches/README.md).

## Agent commands block

`dx init --apply` writes this block between `<!-- dx:begin -->` and
`<!-- dx:end -->` markers in `AGENTS.md`:

```text
<!-- dx:begin -->
- Start: `tools/dx worktree add <branch>`. Finish: `tools/dx worktree release`.
- Fix first: `tools/dx check --apply --affected`.
- Then verify: `tools/dx test --affected`.
- One file: `tools/dx test <file>`.
- One test: `tools/dx test <label> -- --test_filter=<name>`.
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
download the matching standalone binary from the GitHub release into
`<cache>/bin/<version>/`, check it against the sha256 pinned in `.dx/version` for
that platform, and run it. The launcher never downloads a hash file, so a
swapped release asset and its `.sha256` cannot pass. A cached binary is only
run. No Bazel server starts.

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
A binary that does not match its pinned sha256 is deleted and never run.

The launchers are the same for every dx version. `.dx/version` holds the
version on line 1, then one `<sha256>  dx-<os>-<arch>` line per release binary:

```text
1.2.3
<sha256>  dx-linux-x86_64
<sha256>  dx-macos-arm64
```

The version may only use `0-9`, `A-Z`, `a-z`, `.`, `+` and `-`, and may not
start with `.`. Until dx ships release pins, `dx init` writes no hash lines.
Without a line for the platform the launcher does not download: it exits `1`
with the `bazel run @rules_dx//:dx -- ...` command. A binary already in the
cache still runs.

Run from another checkout of the same git repo (a `dx worktree` slot or a
`git worktree`), the launcher first runs that checkout's own `tools/dx`, so that
checkout's `.dx/version` pin applies. `worktree hook` commands never hand off.
Separate clones, other repos, and machines without `git` are not handed off.
Existing launchers gain this on their next upgrade.

When `.dx/version` is missing or has an invalid version, the launcher exits `1`
and prints:

```text
dx: .dx/version is missing; refresh it: bazel run @rules_dx//:dx -- init --apply
dx: .dx/version has an invalid version '<v>'; refresh it: bazel run @rules_dx//:dx -- init --apply
```

`dx init --apply` writes a missing launcher and rewrites a dx launcher that
differs from the current text. dx treats a launcher as its own when it starts
with the template's first three lines (`#!/bin/sh`, `set -eu`,
`DX_PINS=.dx/version`, or `@echo off`, `setlocal`, `set "DX_PINS=.dx\version"`),
or the older `VERSION=` form. Any other launcher is yours and is never
touched. A dx launcher is rewritten when stale, so hand edits to it are lost.
When `.dx/version` is not semver, `dx init --apply` fails with `init_failed`
before writing any file. `dx upgrade --apply` refreshes existing launchers the same way.

## Git hooks

dx does not install or manage Git hooks. For a commit or push hook, write a
plain hook or use a hook tool, and have it run:

```sh
bazel run @rules_dx//:dx -- check //...
```

Enforce the same check in CI. See [GitHub CI](../../github-ci.md).

## Existing Bazel repos

To adopt `rules_dx` in a repo that already uses upstream rules, see
[Adopting rules_dx In An Existing Bazel Repo](../../adopting-existing-bazel.md).

## Bazel preset

`dx init --apply` writes the execution preset as three generated layers,
`.dx/bazelrc/correctness.bazelrc`, `performance.bazelrc` and `resources.bazelrc`,
and puts three `try-import %workspace%/.dx/bazelrc/<layer>.bazelrc` lines at the top
of the root `.bazelrc` (created when missing; its other bytes, BOM and line
endings stay as they are). The layers are gitignored. Before each command, dx writes
any imported layer that is missing or out of date, so a fresh clone needs no extra step.
Plain `bazel` skips layers that do not exist.

- A layer file is created when missing and rewritten when it carries the dx
  header and differs from this dx. A file without the header, or a symlink, is
  left alone with a note.
- Opt out of a layer by commenting its import out (`# try-import ...`). Commenting
  out all three drops the preset; the files may then be deleted. Imports are added
  only when no line, active or commented, names a `.dx/bazelrc` layer. If some
  are present and one is missing, check mode reports it and apply prints a note.
- A workspace set up before the layers were generated keeps working. Apply changes
  each active `import %workspace%/.dx/bazelrc/<layer>.bazelrc` line to `try-import`,
  leaving every other byte as it is, and adds `/.dx/bazelrc/` to an existing
  `.gitignore` that lacks a line covering it. Check mode reports `.bazelrc` and
  `.gitignore` as stale. If git lists files under `.dx/bazelrc`, both modes print a
  note to run `git rm --cached -r .dx/bazelrc`; dx never runs it.
- If the root `.bazelrc` repeats lines the preset owns, or is a symlink, apply
  prints a note and adds no imports.
- In rules_dx itself the preset stays in `tools/bazelrc` and `dx init` does nothing.
