# `dx init`

Use `dx init` to start a new rules_dx workspace, or to add dx to a tree you already have.

In an empty directory, with `dx` installed as in the [tutorial](../../tutorial.md#install):

```sh
dx init --apply
tools/dx test //...
```

In an existing tree, check first, then write:

```sh
dx init
dx init --apply
dx init --apply my-app
dx init --apply --agents claude --ci github
dx init --apply --accept-license visual-studio-build-tools
```

Once the launcher exists, run `tools/dx init` to see drift.

## Usage

```text
dx init [--apply] [--agents <list>] [--ci <provider>] [--no-generate] [--accept-license <id>] [dir]
```

`dx init` checks by default and writes nothing. `--apply` writes the files.

## Flags

| Flag | What it does |
| --- | --- |
| `--apply` | Write the files instead of only checking. |
| `--agents <list>` | Add the files of these agent tools, as a comma list: `claude`, `codex`, `gemini`, `copilot`. |
| `--ci <provider>` | Write the CI files of this provider: `github`, `gitlab` or `none`. |
| `--no-generate` | With `--apply`, stop after the writes and skip `dx generate`. |
| `--accept-license <id>` | With `--apply`, accept this license id in `MODULE.bazel`. Repeatable. |
| `-o, --output text\|json` | Choose the output format. |
| `--workspace <dir>` | Use this workspace directory. |
| `-q, --quiet` | Hide summaries. |
| `-v, --verbose` | Show more logs. |
| `--color auto\|always\|never` | Set color output. |
| `--build-slots <auto\|n\|off>` | Set build slots: `auto`, a count, or `off`. |
| `--build-slots-wait <duration>` | Give up waiting for a build slot after this long. `0` fails at once. |
| `--bazel-startup-option <token>` | Pass one Bazel startup option. Repeatable. |

## Scopes

One optional directory, relative to the workspace. It defaults to the current directory. `--apply` creates it when it is missing. Bazel labels and patterns are not scopes. A second directory is a usage error.

The module name is the directory name, lowercased, with every run of other characters folded to `_`. For the current directory it is `my_project`. An absolute path, a `..` segment, or a name with no `[a-z0-9]` character fails before any file is written.

## New and existing trees

A directory is fresh when it is missing or holds only `.git`, `README*`, `LICENSE*`, `.gitignore`, `.gitattributes`, `.editorconfig`, `.github/` or `.gitlab-ci.yml`. In a fresh directory, init also writes a `MODULE.bazel` that depends on `rules_dx` and a `.bazelversion`. `MODULE.bazel` is the file where a Bazel module declares its name and dependencies.

A tree with other files keeps its own `MODULE.bazel`. If it has none, `--apply` names the entry that made it an existing tree, for example `dx: src/ exists, treating this as an existing repo`.

You choose no language up front. dx wires each language when its sources appear.

Init writes every path in the [fixed file set](#fixed-file-set). You can rerun `dx init --apply` at any time. It keeps your edits outside the parts dx owns.

When the writes succeed, `--apply` runs `dx generate --apply` over the directory. That writes every `BUILD.bazel` file and every missing lockfile. A `BUILD.bazel` file declares the Bazel targets of one directory. The output and exit code then follow `dx generate`. It does not run when a write fails, or with `--no-generate`.

Init writes no `dx.toml`, no root tool config such as `ruff.toml` or `biome.json`, and no `security.toml` or `license.toml`. Those files are yours. Without `dx.toml` every family runs at the strict tier. To opt down, write `dx.toml` with a top-level `tier = "standard"`; see the [config reference](config.md). Init then writes the [`.dx` tree](generate.md#the-dx-tree) from `dx.toml`, or from the defaults when there is none, for the languages it finds.

## Licenses

`--accept-license <id>` appends `dx_licenses.accept(ids = [...], terms_sha256 = {...})` to `MODULE.bazel` with each given id that no `dx_licenses.accept` tag lists yet. When no name binds the extension, it first adds `dx_licenses = use_extension("@rules_dx//core:extensions.bzl", "dx_licenses")`.

`terms_sha256` records the hash of each license's terms as `core/licenses/pins.json` pins them today. When the vendor changes the terms and rules_dx updates the pin, the build fails with `dx_licenses: terms of <id> changed: <url>; re-accept with <tag>` until you read the new terms and replace the tag. A tag with `ids` alone stays valid.

The ids are `android-sdk-license`, `apple-xcode-license`, `cuda-eula`, `visual-studio-build-tools` and `windows-sdk-buildtools`. An unknown id, or the flag without `--apply`, is a usage error.

When a language in the tree needs a license id on this host that is neither given nor in `MODULE.bazel`, `--apply` writes nothing and fails:

```text
dx: init_failed: dx init: cc needs license visual-studio-build-tools; accept it with --accept-license visual-studio-build-tools
```

## CI files

`--ci github` writes the three `.github/workflows` files. `--ci gitlab` writes one `.gitlab-ci.yml` that includes the GitLab template at the pinned rules_dx commit. `--ci none` writes no CI file.

Without `--ci`, init keeps the provider of an existing `.gitlab-ci.yml` or `.github/workflows/ci.yml`. Otherwise it picks `gitlab` when the `origin` remote URL names GitLab, and `github` in every other case.

With the GitHub files, `dx init --apply` ends with `next:` lines for the settings they need:

- require the `dx-ci (aggregate)` check;
- set the `DX_REMOTE_CACHE` variable and the `BUILDBUDDY_API_KEY` secret;
- let Actions create and approve pull requests;
- set the `DX_UPDATE_APP_ID` and `DX_UPDATE_APP_PRIVATE_KEY` secrets.

The lines are fixed text. Init makes no network or `gh` calls.

The CI caller pins the `rules_dx` reusable workflow at a reviewed commit.

## Agent files

Init writes agent tool files only for the agents you name with `--agents`. Without it, init writes the `AGENTS.md` block and no tool file. To add a tool's files later, run `dx init --apply --agents <tool>`. Running it again changes nothing.

Each agent gets the `dx worktree` hooks:

| Agent | Hooks file | Hooks |
| --- | --- | --- |
| Claude Code (`claude`) | `.claude/settings.json`, and `CLAUDE.md` | all four |
| Cursor | `.claude/settings.json`, through Third-Party Imports | all four |
| Codex (`codex`) | `.codex/hooks.json` | `guard`, after you trust it with `/hooks` |
| Gemini CLI (`gemini`) | `.gemini/settings.json` | `guard`, in a trusted folder |
| GitHub Copilot (`copilot`) | `.github/copilot-instructions.md`, and `.github/hooks/dx.json` | `guard` |

See [Hooks](worktree.md#hooks). The hooks run `tools/dx`. On Windows, that also works in a Git Bash or MSYS2 shell.

Init also writes a marked block to `AGENTS.md` and creates the file when it is missing; see [Agent commands block](#agent-commands-block). With `--agents claude`, it writes `CLAUDE.md` with an `@AGENTS.md` import when the import is missing.

A new `.claude/settings.json` also carries a `permissions.allow` list for the read-only commands `tools/dx check *`, `test *`, `build *` and `status *`. When a hooks file already exists, the missing hook and allow entries are added in place, and your other entries stay.

## Ignore files and editor settings

The `.gitignore` lists `target/`, `/bazel-*`, `node_modules/`, `__pycache__/`, `*.pyc`, `.gradle/`, `/build/`, `/.dx/bazelrc/`, `/.dx/local/docs/`, `/.dx/local/results.json`, `/.dx/local/timings/`, `/.dx/local/fuzz/`, `/.dx/local/advisory/`, `/.dx/local/bench/`, `/.dx/local/` and `/.vscode/.rules_rust_analyzer/`. rules_rust's rust-analyzer discovery writes its warnings log under `.vscode/.rules_rust_analyzer/`.

The `.bazelignore` lists each `.dx` scratch directory, `.dx/local` and `.dx/bazelrc`, and leaves `.dx/config` visible to Bazel. An existing `.bazelignore` is left alone. If it lists neither `.dx` nor `.dx/local`, `dx init --apply` prints this to stderr:

```text
dx: .bazelignore lacks .dx/local; add the dx lines so Bazel skips dx state
```

The editor settings point at the managed interpreter and tools, with host-native names. For Rust, they point rust-analyzer at the `rules_rust` discover and flycheck launchers that `dx sync` copies to `.dx/local/ide/rust_analyzer/`. Discover and flycheck each use their own Bazel output base under `.dx/local/ide/`. An output base is the directory where Bazel keeps its build outputs. The file watcher and search exclude `bazel-*` and `.dx/local/ide/`. The settings configure language servers, not debuggers. See [Supported Surface](README.md#supported-surface).

## Output

Without `--apply`, init checks every file it would write. It flags:

- a missing preset import;
- a dx-written launcher that differs from the current launcher text;
- an empty or non-semver `.dx/pins`;
- an `AGENTS.md` without the current dx block;
- a hooks file that lacks dx hooks;
- a `CLAUDE.md` without the `@AGENTS.md` import.

It looks at the agent tool files only when they exist or `--agents` names their agent, so a deleted tool file is not drift.

A missing `.dx/pins`, `.gitignore`, `.bazelignore`, `MODULE.bazel`, `tools/dx` or `tools/dx.cmd`, or a missing file of an agent named with `--agents`, is missing drift. Any other planned file that is gone, such as `AGENTS.md`, `.vscode/settings.json` or `.github/workflows/ci.yml`, is absent.

When nothing is missing, stale, absent or invalid and the pin is valid, the check prints `init current: <module>` to stdout.

When only absent paths remain, it prints this to stderr and exits `0`:

```text
init absent: <paths> (run `dx init --apply` to recreate)
```

Otherwise it prints the parts that apply of this line to stderr, then the `dx init --apply` command to run, or `dx init <dir> --apply` when you gave a directory:

```text
dx: init_failed: init drift: missing <paths>; stale <paths>; absent <paths>; <path> lacks hooks: <hooks>; CLAUDE.md lacks @AGENTS.md; invalid .dx/pins ("<pin>")
```

`--apply` adds the missing hooks to an existing hooks file. It does not fix an invalid pin: write the module version to `.dx/pins`, which the message names.

`--apply` checks every destination before the first write. These fail with `init_failed` and write nothing: a path whose parent is a file, such as a `.vscode` file; an unreadable hooks file; an `AGENTS.md` with a begin marker and no end marker.

`--apply` then writes every file. It prints `wrote <path>` to stdout for each file it creates or refreshes, including `wrote AGENTS.md` when the block changes and `wrote CLAUDE.md` when the import is added. For each scaffold or launcher file that already exists and is kept, it prints this to stderr:

```text
dx: <path> (absent-only, left untouched)
```

`--quiet` hides the stdout lines. The stderr lines still print. When a write fails, `--apply` still lists the files it already wrote, then the `init_failed` error. Closing stdout early does not stop the writes.

`--output json` covers the check only. It reports absent paths as an `init_absent` notice and the plan as `init_plan` events; see [Init plan](#init-plan). `--apply` refuses `--output json` and prints text.

`dx init` writes no reports.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | The check passed, perhaps naming absent paths on stderr, or `--apply` succeeded. |
| `1` | The check found drift, `--apply` failed on content dx refuses, or a needed license id is not accepted. |
| `2` | Usage error, including an extra positional, an unknown flag, or an invalid directory. |
| `3` | `--apply` could not read, write or start something. |
| `141` | `--apply` wrote every file but stdout closed early. |

## Fixed file set

`dx init --apply` writes these paths. The "dx owns" column says what dx still
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
| `tools/bazel`, `tools/bazel.bat` | whole | Written when missing. Rewritten when it differs from the current [Bazel wrapper](#bazel-wrapper) text. A wrapper dx did not write is kept, and `dx status` warns about it. |
| `.dx/bazelrc/correctness.bazelrc`, `.dx/bazelrc/performance.bazelrc`, `.dx/bazelrc/resources.bazelrc` | whole | Generated from `dx.toml`, not committed. Rewritten when they differ, by `dx init --apply`, `dx upgrade --apply` and every other dx command except `status`. The correctness layer sets `--@rules_dx//config:tool_<tool>=@rules_dx//config:off_tool` for each tool no selected family runs, so Bazel never fetches it. When no family found in the tree or named in `dx.toml` needs a C or C++ toolchain, the layer also sets `--@rules_dx//config:cc_toolchains=false` and turns off Bazel's host C and C++ toolchain detection on Linux and macOS, so Bazel fetches no C or C++ toolchain there. Add `[families.cc]` to `dx.toml` to keep them, for example for a Python sdist with native code. |
| `.bazelrc` | lines | Created when missing. Three `try-import %workspace%/.dx/bazelrc/<layer>.bazelrc` lines are added at the top when none exists, and `try-import %workspace%/user.bazelrc` at the bottom when no line names it. Other lines stay. |
| `MODULE.bazel` | lines | Written in a fresh directory. |
| `.bazelversion` | once | Written in a fresh directory only. |
| `.gitignore` | lines | Written when missing. An existing file gets each line of the list above that no line covers appended. |
| `.bazelignore` | lines | Written when missing. |
| `.envrc` | once | Written when missing. Never rewritten. Holds `PATH_add .dx/local/bin` and `watch_file .dx/local/current`; it never runs dx. When the file exists, `dx init --apply` prints those two lines to add to it. |
| `.vscode/extensions.json` | lines | Written when missing. Recommended extension ids that are missing are added, and the lint and format extensions of deselected tools are removed; other ids stay in order. |
| `.dx/BUILD.bazel` | whole | Generated from `dx.toml` (see the [config reference](config.md#dxtoml)): the workspace policy for the families found in the tree, and a `<tool>_config` target for each root tool config. The correctness layer selects it with `--@rules_dx//config:workspace=//.dx:policy`. A flag in your root `.bazelrc` after the imports wins. |
| `.dx/config/` | whole | Generated from `dx.toml`: a `base.<ext>` for each tool of a family found in the tree, on the tier `dx.toml` picks. dx removes the files under `.dx` that it does not generate. |
| `BUILD.bazel` | once | Written when the root has neither `BUILD` nor `BUILD.bazel`. When the root has tool configs such as `ruff.toml`, dx keeps a marked `# dx:begin exports` block in it and leaves the rest alone. |
| `.devcontainer/devcontainer.json` | pin | Written when missing. Only the `image` and the bazel feature version are updated, and only while they still name the image and feature dx wrote. Another image or feature is kept. |
| `.editorconfig`, `.gitattributes` | once | Written when missing. Never rewritten. `.editorconfig` sets LF line endings, a final newline, UTF-8, and indent sizes per language. `.gitattributes` sets `* text=auto eol=lf` and CRLF for `*.bat`, `*.cmd` and `*.ps1`. |
| `.vscode/settings.json` | keys | Written when missing. Only the keys dx sets are updated; comments, other keys and order stay. |
| `.github/workflows/ci.yml` | pin | Written when missing. Only the `uses:` pin of the rules_dx workflow is updated. |
| `.gitlab-ci.yml` | pin | Written with `--ci gitlab` when missing. Only the commit in the template `remote:` URL is updated. |
| `.github/workflows/dx-update.yml` | once | Written when missing. Never rewritten. |
| `.github/workflows/release.yml` | pin | Written when missing and a BUILD file calls `deployment` or a `*_deploy` macro from `@rules_dx//deploy:defs.bzl`; run init again after adding the first one. Only the `uses:` pin is updated. Calls `reusable-release.yml` on `v*` tags. A `workflow_dispatch` takes `publish` (default `false`), so a branch dispatch only plans and dry-runs. Existing files keep their triggers; add `publish` by hand. |
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

When the root has a `MODULE.bazel`, the check also prints a read-only plan of
what adding rules_dx would change, read from the declared files. It runs no Bazel
and writes nothing. A toolchain is the set of tools Bazel picks to build one language
for one platform. Sections:

- `override conflicts`: a `*_override` you own on a rules_dx dependency. Advisory.
- `toolchain conflicts`: toolchains you register that compete with registrations the snippet
  would add, measured against the merged file, so a repo that already has the snippet shows
  none. A Rust toolchain (your `rust.toolchain` tag, or a registered repo from your own
  `rules_rust` extension) is hard. A `toolchains_llvm` / `cc_configure` registration is
  advisory, like `python.toolchain`: yours wins and nothing needs to change.
  Bazel prefers root-module registrations in listed order, first wins, and
  `--extra_toolchains` beats all of them. Registrations of repos dx cannot link to an
  ecosystem are never flagged.
- `bazelrc conflicts`: a line in the root `.bazelrc`, or a file it imports through
  `%workspace%/`, that exactly repeats a flag line the dx preset layers own. Advisory; dx
  does not import the layers while the root file repeats them, so drop the line.
- `workspace_ignored`: a `WORKSPACE` or `WORKSPACE.bazel` in the root. Bazel 9 does not read
  it, so its repos must move to `MODULE.bazel`; dx does not translate the file. Advisory, and
  reported even when there is no `MODULE.bazel` yet: check prints
  `advisory: WORKSPACE is not read by Bazel 9; move its repos to MODULE.bazel (WORKSPACE:1)`
  to stderr, and `--apply` adds it to its notes.

Each finding names the file and line, including `include()`d `.MODULE.bazel` files. Hard
findings add an `init plan: N hard conflicts (...)` clause to `init drift` and exit `1`.
Advisory findings are printed and never change the exit code.

With `--output=json`, the plan is one `init_plan` event per finding instead of text, with
`kind` (`override_conflict`, `toolchain_conflict`, `bazelrc_conflict`, `workspace_ignored`),
`subject`, `before`, `after`, `path`, `line` (left out when unknown), `hint`, and `strength`
(`hard` or `advisory`). Plan notes, such as a `MODULE.bazel` that could not be compared, are `notice` events
with code `init_plan_partial`.

## Agent commands block

`dx init --apply` writes this block between `<!-- dx:begin -->` and
`<!-- dx:end -->` markers in `AGENTS.md`:

```text
<!-- dx:begin -->
- Start: `tools/dx worktree add <branch>`. Finish: `tools/dx worktree release`. Work in the slot it prints, not in the main checkout.
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
`dx status` reports a missing or out-of-date block, a missing `@AGENTS.md`
import, and a missing hook in its `init` check.

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
arm64. The download launcher exits `1` and prints the `bazel run @rules_dx//dx/launch:dx -- ...`
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
start with `.`. `dx init` writes no hash lines.
Without a line for the platform the launcher does not download: it exits `1`
with the `bazel run @rules_dx//dx/launch:dx -- ...` command. A binary already in the
cache still runs.

The launchers read the version from the one top-level `bazel_dep` named
`rules_dx` in `MODULE.bazel`. It may span several lines and carry `repo_name`,
a trailing comma or `#` comments. With no such dep, two of them, or an override
of `rules_dx`, dx runs through Bazel instead.

Set `DX_RELEASE_BASE_URL` to download from a mirror, as with the
[shim](README.md): `<url>/v<version>/dx-<os>-<arch>`, `https://` or `http://`
to `127.0.0.1`, `[::1]` or `localhost`. The pinned sha256 still applies. The
Windows launcher also takes only plain URL characters there.

Run from another checkout of the same git repo (a `dx worktree` slot or a
`git worktree`), the launcher first runs that checkout's own `tools/dx`, so that
checkout's `.dx/pins` pin applies. `worktree hook` commands never hand off.
Separate clones, other repos, and machines without `git` are not handed off.

When `.dx/pins` is missing or has an invalid version, the launcher exits `1`
and prints:

```text
dx: .dx/pins is missing; refresh it: bazel run @rules_dx//dx/launch:dx -- init --apply
dx: .dx/pins has an invalid version '<v>'; refresh it: bazel run @rules_dx//dx/launch:dx -- init --apply
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

## Bazel wrapper

Bazelisk runs `tools/bazel` on Linux and macOS and `tools/bazel.bat` on Windows
in place of Bazel. The wrapper first runs
`tools/dx worktree hook bazel -- <args>`, which refuses a Bazel `clean` in a
slot and applies [memory admission](worktree.md#memory-admission). Then it runs `$BAZEL_REAL`
with the same arguments. The hook's output goes to stderr.

Only a refusal (exit `77`) or a stop (exit `130` or `143`) from the hook stops
Bazel, and the wrapper exits with that code. On any other failure the wrapper
prints a warning to stderr and runs Bazel anyway. Such failures include a
`tools/dx` that is missing, a dx too old for the hook, and a launcher that
cannot run dx (unsupported libc, `HOME` unset).

The wrapper skips the hook when `DX_BAZEL_CHILD` is set. dx sets
`DX_BAZEL_CHILD=1` on every Bazel it starts. The wrapper exits non-zero when
`BAZEL_REAL` is unset, as it is without Bazelisk.

dx treats a wrapper as its own when it starts with the template's first three
lines. `dx status` warns about a `tools/bazel` or `tools/bazel.bat` dx did not
write. On Windows, Bazelisk runs a `tools/bazel.ps1` in place of
`tools/bazel.bat`, so `dx status` also warns about a `tools/bazel.ps1` that does
not run `worktree hook bazel`.

## Git hooks

Init and `dx worktree add` do not install Git hooks. For a commit or push hook, write a plain hook or use a hook tool, and have it
run:

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
