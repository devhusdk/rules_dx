# `dx init`

Empty directory, with Bazel. Create `.bazelversion`, an empty `BUILD.bazel`, and a `MODULE.bazel`
with `bazel_dep(name = "rules_dx", version = "<release>")`. Then:

```sh
bazel run @rules_dx//:dx -- init --apply
tools/dx test //...
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
dx init [--apply] [--agents <list>] [--ci <provider>] [dir]
```

Starts a new workspace or adopts `dx` into an existing tree. Checks by
default; `--apply` scaffolds. Takes an optional directory, relative to the
workspace and defaulting to the current one. `--apply` creates the directory
when it is missing. A directory is fresh when it is missing or holds only `.git`,
`README*`, `LICENSE*`, `.gitignore`, `.gitattributes`, `.editorconfig` or
`.github/` or `.gitlab-ci.yml`: init then also writes a `MODULE.bazel` that depends on `rules_dx`
and a `.bazelversion`. A tree with other files keeps its own `MODULE.bazel`.
If such a tree has no `MODULE.bazel`, `--apply` names the entry that made it an
existing tree, for example `dx: src/ exists, treating this as an existing repo`.
`init --apply` also adds the `rules_dx_<family>` dep for each language family found in the tree, once that module is published.
Nothing language-specific is chosen up front; languages are wired when their
sources appear.

The module name is the directory name, lowercased, with every run of other
characters folded to `_`, or `my_project` for the current directory. An
absolute path, a `..` segment, or a name with no `[a-z0-9]` character fails
before any file is written.

It writes every path in the [fixed file set](#fixed-file-set). Re-running
`dx init --apply` is safe: it keeps your edits outside the rows owned by `dx`.

Init writes no `dx.toml`, no root tool config such as `ruff.toml` or `biome.json`, and no `security.toml` or `license.toml`. Those files are yours. To pick the strict tier, write `dx.toml` with a top-level `tier = "strict"` (see the [config reference](config.md)). Init then writes the [`.dx` tree](generate.md#the-dx-tree) from `dx.toml`, or from the defaults when there is none, for the languages found.

`--ci` picks the CI files. `github` writes the three `.github/workflows` files, `gitlab` writes one `.gitlab-ci.yml` that includes the GitLab template at the pinned rules_dx commit, and `none` writes no CI file. Without it init keeps the provider of an existing `.gitlab-ci.yml` or `.github/workflows/ci.yml`, then picks `gitlab` when the `origin` remote URL names GitLab, else `github`.

Init writes agent tool files only for the agents you name with `--agents=claude,codex,gemini,copilot` (a comma list). Without it, init writes the `AGENTS.md` block and no tool file. Run `dx init --apply --agents <tool>` later to add a tool's files to an existing repository; running it again changes nothing. Each agent gets the `dx worktree` hooks:

| Agent | Hooks file | Hooks |
| --- | --- | --- |
| Claude Code (`claude`) | `.claude/settings.json`, and `CLAUDE.md` | all four |
| Cursor | `.claude/settings.json`, through Third-Party Imports | all four |
| Codex (`codex`) | `.codex/hooks.json` | `guard`, after you trust it with `/hooks` |
| Gemini CLI (`gemini`) | `.gemini/settings.json` | `guard`, in a trusted folder |
| GitHub Copilot (`copilot`) | `.github/copilot-instructions.md`, and `.github/hooks/dx.json` | `guard` |

See [Hooks](worktree.md#hooks). The hooks run `tools/dx`, which also runs in a Git Bash or MSYS2 shell on Windows. It also writes a marked block to `AGENTS.md`,
creating the file when it is missing (see below). With `--agents=claude` it writes `CLAUDE.md` with an
`@AGENTS.md` import when the import is missing.
The new `.claude/settings.json` also carries a `permissions.allow` list for the
read-only commands `tools/dx check *`, `test *`, `build *`,
and `status *`. If a hooks file
already exists, the missing hook and allow entries are added in place and your
other entries stay.

The CI caller pins the
`rules_dx` reusable workflow at a reviewed commit. The `.gitignore` lists `target/`, `/bazel-*`, `node_modules/`,
`__pycache__/`, `*.pyc`, `.gradle/`, `/build/`, `/.dx/bazelrc/`,
`/.dx/local/docs/`, `/.dx/local/results.json`, `/.dx/local/timings/`, `/.dx/local/fuzz/`, `/.dx/local/advisory/`, `/.dx/local/bench/`, `/.dx/local/` and
`/.vscode/.rules_rust_analyzer/` (rules_rust's rust-analyzer discover writes its
warnings log under `.vscode/.rules_rust_analyzer/`). The `.bazelignore` lists each `.dx` scratch directory (`.dx/local`, `.dx/bazelrc`) and leaves `.dx/config` visible to Bazel. When a `.bazelignore` already exists it is left alone, and
`dx init --apply` prints `dx: .bazelignore lacks .dx/local; add the dx lines so Bazel skips dx state`
to stderr if it lists neither `.dx` nor `.dx/local`. The editor settings point at the managed
interpreter and tools with host-native names. For Rust they point
rust-analyzer at the `rules_rust` discover and flycheck launchers that `dx sync` copies to
`.dx/local/ide/rust_analyzer/`. Discover and flycheck use separate Bazel output bases under `.dx/local/ide/`, and the file
watcher and search exclude `bazel-*` and `.dx/local/ide/`. They configure language servers, not debuggers. See
[Supported Surface](README.md#supported-surface).
Flags: `--apply`, `--agents <list>`, `--ci github|gitlab|none`.
Scopes: optional single directory, created by `--apply` when missing.

Output: text only. Check mode looks for every file init writes.
It also flags a missing preset import. It flags a dx-written launcher that differs from the current launcher text
and an empty or non-semver `.dx/pins`. It flags an `AGENTS.md` without the current dx block, a hooks file that lacks dx hooks, and a
`CLAUDE.md` without the `@AGENTS.md` import. It looks at the tool files only when they exist or `--agents` names their agent, so a deleted tool file is not drift.
A missing `.dx/pins`, `.gitignore`, `.bazelignore`, `MODULE.bazel`, `tools/dx`, `tools/dx.cmd`, or a missing file of an agent named with `--agents` is missing drift.
Any other planned file that is gone, such as `AGENTS.md`, `.vscode/settings.json` or `.github/workflows/ci.yml`, is absent.
When nothing is missing, stale, absent or invalid and the pin is valid, it prints
`init current: <module>` to stdout. When only absent paths remain, it prints
``init absent: <paths> (run `dx init --apply` to recreate)`` to stderr and exits `0`. JSON output reports absent paths as an `init_absent` notice.
Otherwise it prints
`dx: init_failed: init drift: missing <paths>; stale <paths>; absent <paths>; <path> lacks hooks: <hooks>; CLAUDE.md lacks @AGENTS.md; invalid .dx/pins ("<pin>")`
to stderr (only the parts that apply), followed by the `dx init --apply` command to run
(`dx init <dir> --apply` when a directory was given). Apply adds the missing hooks to an existing hooks file. An invalid pin is not fixed by apply: write the module version to
`.dx/pins`, which the message names.
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

Exit codes: `0` check passed (absent paths may be named on stderr) or apply succeeded, `2` usage error, including
extra positionals, an unknown flag, or an invalid directory, `1` check found drift or apply
failed, `141` apply wrote every file but stdout closed early.

## Fixed file set

`dx init --apply` writes these paths. The `dx owns` column says what dx still
touches after you edit the file:

- `whole`: dx rewrites the whole file to match the running dx. Do not edit it.
- `extends`: the file is yours and extends a base dx owns. Only the base is rewritten.
- `keys`: the file is yours. dx sets only its own keys; comments, other keys and order stay.
- `block`: the file is yours. dx rewrites only its marked block.
- `lines`: the file is yours. dx adds its missing lines and leaves the rest.
- `pin`: the file is yours, except the rules_dx pin dx keeps current.
- `once`: dx writes the file when it is missing and never rewrites it.

`dx status --files` lists every planned file with its label and state.

A file that dx creates and whose format has comments starts with a one-line `dx:` comment that says what dx does to it. JSON and Markdown files, `.bazelversion` and `.dx/pins` have none. A file that already exists keeps its bytes.

| Path | dx owns | What `dx init --apply` updates |
| --- | --- | --- |
| `.dx/pins` | whole | Generated. One `<version> <sha256> <asset>` line per release binary, for the current version only. `dx init --apply` and `dx upgrade --apply` rewrite it when it differs. Edit the `MODULE.bazel` version, not this file. |
| `tools/dx`, `tools/dx.cmd` | whole | Written when missing. Rewritten when it differs from the current launcher text. A launcher dx did not write is kept. On Linux the launcher needs glibc 2.28 or newer and stops with a message on musl (Alpine) or older glibc. |
| `third_party/rules_dx/` | whole | Rewritten on every run: `rules_dx.MODULE.bazel`, the patches, `BUILD.bazel` and `.gitattributes`. |
| `.dx/bazelrc/correctness.bazelrc`, `.dx/bazelrc/performance.bazelrc`, `.dx/bazelrc/resources.bazelrc` | whole | Generated from `dx.toml`, not committed. Rewritten when they differ, by `dx init --apply`, `dx upgrade --apply` and every other dx command except `status`. |
| `.bazelrc` | lines | Created when missing. Three `try-import %workspace%/.dx/bazelrc/<layer>.bazelrc` lines are added at the top when none exists, and `try-import %workspace%/user.bazelrc` at the bottom when no line names it. Other lines stay. |
| `MODULE.bazel` | lines | Written in a fresh directory. Otherwise the `include` of the rules_dx overrides is added after `bazel_dep(name = "rules_dx")`, unless the file overrides a rules_dx module. |
| `.bazelversion` | once | Written in a fresh directory only. |
| `.gitignore` | lines | Written when missing. An existing file gets each line of the list above that no line covers appended. |
| `.bazelignore` | lines | Written when missing. |
| `.envrc` | once | Written when missing. Never rewritten. Holds `PATH_add .dx/local/bin` and `watch_file .dx/local/current`; it never runs dx. When the file exists, `dx init --apply` prints those two lines to add to it. |
| `.vscode/extensions.json` | lines | Written when missing. Recommended extension ids that are missing are added, and the lint and format extensions of tools that are no longer selected are removed; other ids stay in order. |
| `.dx/BUILD.bazel` | whole | Generated from `dx.toml` (see the [config reference](config.md#dxtoml)): the workspace policy for the families found in the tree, and a `<tool>_config` target for each root tool config. The correctness layer selects it with `--@rules_dx//config:workspace=//.dx:policy`. A flag in your root `.bazelrc` after the imports wins. |
| `.dx/config/` | whole | Generated from `dx.toml`: a `base.<ext>` for each tool of a family found in the tree, on the tier `dx.toml` picks. A file under `.dx` that dx no longer generates is removed. |
| `BUILD.bazel` | once | Written when the root has neither `BUILD` nor `BUILD.bazel`. When the root has tool configs such as `ruff.toml`, dx keeps a marked `# dx:begin exports` block in it and leaves the rest alone. |
| `.devcontainer/devcontainer.json` | pin | Written when missing. Only the `image` and the bazel feature version are updated, and only while they still name the image and feature dx wrote. Another image or feature is kept. |
| `.editorconfig`, `.gitattributes` | once | Written when missing. Never rewritten. `.editorconfig` sets LF line endings, a final newline, UTF-8, and indent sizes per language. `.gitattributes` sets `* text=auto eol=lf` and CRLF for `*.bat`, `*.cmd` and `*.ps1`. |
| `.vscode/settings.json` | keys | Written when missing. Only the keys dx sets are updated; comments, other keys and order stay. |
| `.github/workflows/ci.yml` | pin | Written when missing. Only the `uses:` pin of the rules_dx workflow is updated, and a retired `rules_dx_version` input is removed. |
| `.gitlab-ci.yml` | pin | Written with `--ci gitlab` when missing. Only the commit in the template `remote:` URL is updated. |
| `.github/workflows/dx-update.yml` | once | Written when missing. Never rewritten. |
| `.github/workflows/release.yml` | pin | Written when missing. Only the `uses:` pin is updated. Calls `reusable-release.yml` on `v*` tags. A `workflow_dispatch` takes `publish` (default `false`), so a branch dispatch only plans and dry-runs. Existing files keep their triggers; add `publish` by hand. |
| `AGENTS.md` | block | Created when missing. Only the text between `<!-- dx:begin -->` and `<!-- dx:end -->` is rewritten. |
| `CLAUDE.md` | lines | Created when missing and `--agents=claude` is given. `@AGENTS.md` is added when the import is missing. |
| `.github/hooks/dx.json` | keys | Created when missing and `--agents=copilot` is given; missing hook entries are merged in place. |
| `.github/copilot-instructions.md` | lines | Created when missing and `--agents=copilot` is given. A line pointing at `AGENTS.md` is added when the file does not mention it. |
| `.claude/settings.json` | keys | Written when missing and `--agents=claude` is given. When it exists, the missing hooks and `permissions.allow` entries are added in place. |
| `.codex/hooks.json`, `.gemini/settings.json` | keys | Written when missing and the matching `--agents` value is given. When they exist, the guard hook is added if it is missing. |

`.vscode/extensions.json` recommends the Bazel extension, plus the editor extension for each language with files in the tree (Python for `.py`, Go for `.go`, and so on). Edit the file to change the list. `dx init` adds missing ids and removes the lint and format extensions of deselected tools; other ids stay.

Check reports a missing essential path or a stale path as drift, and exits `1`. It reports any other missing planned path as absent and exits `0`. An old dx key in `.vscode/settings.json`, or an old pin in `.github/workflows/ci.yml`, is stale. Deleting a tool file stops check from reporting it, and `dx init --apply` does not write it again unless you pass `--agents`. `dx init --apply` recreates an absent path. To opt out of a preset layer, comment out its import (see
[Bazel preset](#bazel-preset)).

## Init plan

When the root has a `MODULE.bazel`, `dx init` (the check) also prints a read-only plan of
what adopting rules_dx would change, found from the declared files. It runs no Bazel
and writes nothing. Sections:

- `override conflicts`: a `*_override` you own. One on a module dx patches
  (`single_version_override` of `rules_rust`, for example) is hard, because dx cannot add its
  patches; one on another rules_dx dependency is advisory.
- `toolchain conflicts`: toolchains you register that compete with registrations the snippet
  would add, measured against the merged file, so a repo that already has the snippet shows
  none. A Rust toolchain (your `rust.toolchain` tag, or a registered repo from your own
  `rules_rust` extension) is hard. A `toolchains_llvm` / `cc_configure` registration is
  advisory, like `python.toolchain`: yours wins and nothing needs to change.
  Bazel prefers root-module registrations in listed order (first wins) and
  `--extra_toolchains` beats all of them, see
  <https://bazel.build/extending/toolchains>. A `python.toolchain` of yours is advisory:
  rules_dx registers Python only as a dependency module, so yours always wins. Registrations
  of repos dx cannot link to an ecosystem are never flagged.
- `bazelrc conflicts`: a line in the root `.bazelrc`, or a file it imports through
  `%workspace%/`, that exactly repeats a flag line the dx preset layers own. Advisory; dx
  does not import the layers while the root file repeats them, so drop the line.
- `workspace_ignored`: a `WORKSPACE` or `WORKSPACE.bazel` in the root. Bazel 9 does not read
  it, so its repos must move to `MODULE.bazel`; dx does not translate the file. Advisory, and
  reported even when there is no `MODULE.bazel` yet: check prints
  `advisory: WORKSPACE is not read by Bazel 9; move its repos to MODULE.bazel (WORKSPACE:1)`
  to stderr, and `--apply` adds it to its notes.

| Exit code | Meaning |
| --- | --- |
| `0` | no hard findings (advisory findings are printed) |
| `1` | a hard override or toolchain conflict, or other init drift |
| `2` | usage error |

Each finding names the file and line, including `include()`d `.MODULE.bazel` files. Hard
findings add an `init plan: N hard conflicts (...)` clause to `init drift` and exit 1;
advisory findings never change the exit code.

With `--output=json`, the plan is one `init_plan` event per finding instead of text, with
`kind` (`override_conflict`, `toolchain_conflict`, `bazelrc_conflict`, `workspace_ignored`),
`subject`, `before`, `after`, `path`, `line` (left out when unknown), `hint`, and `severity`
(`hard` or `advisory`). Plan notes, such as a `MODULE.bazel` that could not be compared, are `notice` events
with code `init_plan_partial`. `--apply` refuses `--output=json`. The agent default of `json` does not apply to `--apply`, which prints text.

## rules_dx overrides

`dx init --apply` writes the root-only
`single_version_override`s `rules_dx` needs to `third_party/rules_dx/`:
`rules_dx.MODULE.bazel`, one `<module>.<file>.patch` per patch, `BUILD.bazel`
and `.gitattributes`. These files are always rewritten to match the running dx.
When `MODULE.bazel` has `bazel_dep(name = "rules_dx")`, apply adds
`include("//third_party/rules_dx:rules_dx.MODULE.bazel")` after it and prints
`wrote MODULE.bazel`. When
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
- Before a PR: `tools/dx check --affected`. It is the gate.
- One file: `tools/dx test <file>`.
- One test: `tools/dx test <label> -- --test_filter=<name>`.
- Read the `--output=json` result.
- Lint config: edit the tool's own root file; for tools without extends, run `tools/dx config override <tool>`; the tier is `tier` in dx.toml; never edit `.dx/` (generated, drift-checked).
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

`tools/dx` (sh) and `tools/dx.cmd` run dx without Bazel. They read `.dx/pins`,
download the matching standalone binary from the GitHub release into
`<cache>/bin/<version>/`, check it against the sha256 pinned in `.dx/pins` for
that platform, and run it. The launcher never downloads a hash file, so a
swapped release asset and its `.sha256` cannot pass. A cached binary is only
run. No Bazel server starts.

`<cache>` is `DX_CACHE_DIR`, else `$XDG_CACHE_HOME/dx`, else `~/.cache/dx`
(`%USERPROFILE%\.cache\dx` on Windows). The [global shim](README.md#global-shim)
uses the same cache, so each runs binaries the other downloaded.

```text
tools/dx --version
tools/dx check //...
```

Binaries exist for Linux x86_64 and arm64, macOS arm64, and Windows x86_64 and
arm64. The download launcher exits `1` and prints the `bazel run @rules_dx//:dx -- ...`
command to use instead when the download fails or the platform has no binary.
A binary that does not match its pinned sha256 is deleted and never run.

The launchers are the same for every dx version. `.dx/pins` holds the
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
checkout's `.dx/pins` pin applies. `worktree hook` commands never hand off.
Separate clones, other repos, and machines without `git` are not handed off.
Existing launchers gain this on their next upgrade.

When `.dx/pins` is missing or has an invalid version, the launcher exits `1`
and prints:

```text
dx: .dx/pins is missing; refresh it: bazel run @rules_dx//:dx -- init --apply
dx: .dx/pins has an invalid version '<v>'; refresh it: bazel run @rules_dx//:dx -- init --apply
```

`dx init --apply` writes a missing launcher and rewrites a dx launcher that
differs from the current text. dx treats a launcher as its own when it starts
with the template's first three lines (`#!/bin/sh`, `set -eu`,
`DX_PINS=.dx/pins`, or `@echo off`, `setlocal`, `set "DX_PINS=.dx\pins"`),
or the older `VERSION=` form. Any other launcher is yours and is never
touched. A dx launcher is rewritten when stale, so hand edits to it are lost.
When `.dx/pins` is not semver, `dx init --apply` fails with `init_failed`
before writing any file. It also fails when `MODULE.bazel` pins a different
`rules_dx` than the running `dx`. `dx upgrade --apply` refreshes existing launchers the same way.

## Git hooks

dx does not install or manage Git hooks. For a commit or push hook, write a
plain hook or use a hook tool, and have it run:

```sh
tools/dx check //...
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
endings stay as they are) and `try-import %workspace%/user.bazelrc` at the bottom
when no line names `user.bazelrc`. The layers are gitignored. Before each command, dx writes
any imported layer that is missing or out of date, so a fresh clone needs no extra step.
Plain `bazel` skips layers that do not exist.

- A layer file is created when missing and rewritten when it carries the dx
  header and differs from this dx. A file without the header, or a symlink, is
  left alone with a note.
- Opt out of a layer by commenting its import out (`# try-import ...`). Commenting
  out all three drops the preset; the files may then be deleted. Imports are added
  only when no line, active or commented, names a `.dx/bazelrc` layer. If some
  are present and one is missing, check mode reports it and apply prints a note.
- If git lists files under `.dx/bazelrc`, both modes print a note to run
  `git rm --cached -r .dx/bazelrc`; dx never runs it.
- If the root `.bazelrc` repeats lines the preset owns, or is a symlink, apply
  prints a note and adds no imports.
- Warnings are errors by default in the rules_dx wrappers. Add
  `common --@rules_dx//config:warnings_as_errors=false` to the root `.bazelrc` to
  turn that off repo-wide; a target can override it with `warnings_as_errors`.
- Machine-specific flags go in `user.bazelrc`:

  ```sh
  echo 'common --disk_cache=~/.cache/bazel-disk' >> user.bazelrc
  echo 'common --remote_cache=grpcs://cache.example.com' >> user.bazelrc
  echo 'common --credential_helper=cache.example.com=%workspace%/tools/cache-helper.sh' >> user.bazelrc
  echo 'build --jobs=8' >> user.bazelrc
  ```
