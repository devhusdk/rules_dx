# `dx new`, `dx upgrade` And `dx rollback`

```sh
bazel run @rules_dx//:dx -- new --apply rust my_project
bazel run @rules_dx//:dx -- upgrade --from 1.0.0 --to 2.0.0
bazel run @rules_dx//:dx -- rollback
```

## `dx new`

```text
dx new [--apply] <language> [name]
```

Starts a new project. Use `dx init` to adopt an existing tree. Checks by default; `--apply` scaffolds. Scaffolds a minimal project for one language: `rust`, `python`,
`javascript`, `typescript`, `go`, `java`, `kotlin`, `scala`, `csharp`,
`fsharp`, `c`, `cpp`, `rust-web`, `ruby`, `powershell`, `astro`, `svelte`, `vue`, `mdx`, `zig`, `elixir`, `gleam`, `erlang`, or `cuda`. Never overwrites existing files.
There is no `--force`. Runs outside a workspace.

`c` scaffolds a standalone C consumer and `cpp` a standalone C++ consumer.
Both build with the managed LLVM toolchain.

Removed spellings fail with exit `2` and name the replacement:

- `cc` is now `cpp`.
- `c#` and `c-sharp` are now `csharp`.
- `f#` and `f-sharp` are now `fsharp`.

Every starter also writes `.dx/version`, `.devcontainer/devcontainer.json`,
`.gitignore`, `.bazelignore`, `.envrc`, `.vscode/settings.json`, `.vscode/extensions.json`,
`.github/workflows/ci.yml` and `MODULE.bazel.snippet`. Check mode reports
them when they are missing.

Like `dx init --apply`, `--apply` also writes the launcher (`tools/dx` and
`tools/dx.cmd`), the `rules_dx` overrides under `third_party/rules_dx/` (the new
`MODULE.bazel` includes them, see [overrides](init.md#rules_dx-overrides)), the `dx worktree` hooks files (`.claude/settings.json`,
`.codex/hooks.json`, `.gemini/settings.json`), and the
marked block in `AGENTS.md` and the `@AGENTS.md` import in `CLAUDE.md` in the new project. Check mode also requires
`tools/dx` and `tools/dx.cmd`.

Check mode prints this and exits `0` when every file exists:

```text
new current: <language> <name>
```

When files are missing it fails with `new_failed` on stderr and exits `1`:

```text
dx: new_failed: new drift: missing <files> (run `dx new <language> <name> --apply`)
```

`--apply` prints one line per file it writes, and one stderr line per file
that already exists. Paths start with the destination:

```text
wrote <name>/<path>
dx: <name>/<path> (absent-only, left untouched)
```

When a hooks file such as `<name>/.claude/settings.json` already exists without
the `dx worktree` hooks, it is left as is and stderr lists the hooks to add:

```text
dx: <name>/<hooks file> exists; add these hooks: <hooks>
```

Output: text only.

Exit codes: `0` success, `2` usage errors including an unknown language, an
invalid destination or package identity, or extra positionals, `1` missing
files in check mode or scaffolding failed. It also exits `1` with `new_failed`,
before writing any file, when `<name>/.dx/version` already exists and is not
semver. Like `dx init --apply`, `--apply` checks every destination first, so a
parent that is a file or an `AGENTS.md` with an unmatched begin marker fails
before any write, and a later write failure still lists the files written.

The `[name]` is the destination directory and defaults to `my_project`.
Destinations stay inside the workspace. Absolute paths, `..` segments, empty
segments, backslashes, control characters, and the characters `< > : " | ? *`
are rejected. Segments never end with a space or `.`, and reserved device
names such as `CON` and `NUL` are rejected. Nested destinations such as
`teams/demo` are allowed. Spaces and Unicode are allowed in the destination.

The package identity derives from the final destination segment. It is
lowercased. ASCII letters, digits, `.`, `-` and `_` are kept. Every other run
of characters folds to one `-`. Leading and trailing `-`, `_` and `.` are
trimmed. `My App` scaffolds the `my-app` package. The derived identity must satisfy the
language rule:

- `rust` takes Cargo names.
- `python` takes dotted names.
- `javascript` takes npm names.
- `typescript` takes npm names.
- `go` takes module paths.
- `java` takes Maven artifact ids.
- `kotlin` takes Maven artifact ids.
- `scala` takes Maven artifact ids.
- `csharp` takes NuGet ids.
- `fsharp` takes NuGet ids.
- `c` takes Bazel module names.
- `cpp` takes Bazel module names.
- `rust-web` takes Cargo names.
- `ruby` takes Bazel module names.
- `powershell` takes Bazel module names.
- `astro` takes Bazel module names.
- `svelte` takes Bazel module names.
- `vue` takes Bazel module names.
- `mdx` takes Bazel module names.
- `zig` takes Bazel module names.
- `elixir` takes Bazel module names.
- `gleam` takes Bazel module names.
- `erlang` takes Bazel module names.
- `cuda` takes Bazel module names.

A Bazel module name uses lowercase letters, digits, dots, hyphens and
underscores, starts with a letter, and ends with a letter or digit. Rejected
names fail before any file is written.

`rust-web` scaffolds a shared core with native and browser targets:
`Cargo.toml`, `src/lib.rs`, `src/main.rs`, `BUILD.bazel`, `MODULE.bazel`,
`.bazelversion`, and `README.md`.

## `dx new ruby`, `dx new powershell`, `dx new zig`, `dx new elixir`, `dx new gleam` And `dx new erlang`

```text
dx new [--apply] ruby [name]
dx new [--apply] powershell [name]
dx new [--apply] zig [name]
dx new [--apply] elixir [name]
dx new [--apply] gleam [name]
dx new [--apply] erlang [name]
```

Scaffolds a standalone consumer with a library, a binary and a test:
`MODULE.bazel`, `.bazelversion`, `.bazelrc`, `BUILD.bazel`, sources, and
`README.md`. Ruby, PowerShell, Zig, Elixir, Gleam and Erlang come from the rules_dx toolchains, so nothing needs to be
installed. The PowerShell targets are Linux only. The Elixir and Gleam targets need Linux x86_64. The Erlang targets need Linux x86_64, Linux arm64 or macOS arm64. The Zig library imports as `hello`.
The Elixir starter is the OTP application `hello` with sources in `lib/`, an
escript, and an ExUnit test in `test/`.
The Gleam starter has the module `hello` in `src/`, a gleeunit test in `test/`,
and a binary, library and test for both Erlang and JavaScript. The JavaScript
targets end in `_js` and run on Node.js.
The Erlang starter is the OTP application `hello` in `src/`, with an escript
and an EUnit test in `test/`.

```sh
bazel run @rules_dx//:dx -- new --apply ruby demo
```

Build, run and test inside the new project:

```text
bazel build //...
bazel run :demo_bin
bazel test //...
```

Output: text only.

Exit codes: `0` success, `2` usage errors including an invalid
destination or package identity, `1` missing files in check mode or
scaffolding failed.

Scaffolding into an existing directory adds absent files only and leaves
custom files untouched.

## `dx new cuda`

```text
dx new [--apply] cuda [name]
```

Scaffolds a standalone CUDA consumer: `MODULE.bazel`, `.bazelversion`,
`.bazelrc`, `BUILD.bazel`, `scale.cu`, `scale.cuh`, `scale.h`, `main.cc`,
`scale_test.cc`, and `README.md`. Linux x86_64 only.

rules_cuda and toolchains_llvm only read their tags from the root module, so
`MODULE.bazel` carries the pinned CUDA 13.4 toolkit components and the LLVM 22
host compiler for nvcc. No CUDA install is needed to build.

The stanza also pins cuBLAS, cuFFT and cuRAND. Bazel fetches them only when a
target depends on `//cuda/components:cublas`, `:cufft` or `:curand`. A
workspace with an older stanza must copy the stanza again to use them.

The stanza also contains `cuda_license.license(accept = ["cuda-eula"])`, which
accepts the [NVIDIA CUDA Toolkit EULA](https://docs.nvidia.com/cuda/eula/index.html)
for the build. Remove that line to decline; CUDA targets then fail to build.

The test checks host-side logic and launches no kernel, so `bazel test //...`
passes without a GPU. The binary runs the kernel when a GPU is present.

`dx init` adds the same stanza to `MODULE.bazel.snippet` when the workspace has
`.cu` or `.cuh` files.

Output: text only.

Exit codes: `0` success, `2` usage errors including an invalid destination or
module name, `1` missing files in check mode or scaffolding failed. It also exits `1` with `new_failed`,
before writing any file, when `<name>/.dx/version` already exists and is not
semver. Like `dx init --apply`, `--apply` checks every destination first, so a
parent that is a file or an `AGENTS.md` with an unmatched begin marker fails
before any write, and a later write failure still lists the files written.

## `dx new c` And `dx new cpp`

```text
dx new [--apply] c [name]
dx new [--apply] cpp [name]
```

Scaffolds a standalone consumer with a library, a binary and a test:
`MODULE.bazel`, `.bazelversion`, `.bazelrc`, `BUILD.bazel`, `hello.h`,
`README.md`, and the sources. `c` writes `hello.c`, `main.c` and
`hello_test.c` and compiles them as C. `cpp` writes `hello.cc`, `main.cc` and
`hello_test.cc` and compiles them as C++. Both use the standard library only.
The compiler comes from the managed LLVM toolchain, so nothing needs to be
installed. A destination such as `1demo` is rejected because the module name
must start with a letter.

```sh
bazel run @rules_dx//:dx -- new --apply c demo
```

Build, run and test inside the new project:

```text
bazel build //...
bazel run :demo_bin
bazel test //...
```

Output: text only.

Exit codes: `0` success, `2` usage errors including a removed spelling such as
`cc`, an invalid destination or package identity, `1` missing files in check
mode or scaffolding failed.

Scaffolding into an existing directory adds absent files only and leaves
custom files untouched.

## Framework Starters

```text
dx new [--apply] astro [name]
dx new [--apply] svelte [name]
dx new [--apply] vue [name]
dx new [--apply] mdx [name]
```

Scaffolds a standalone consumer with one component, a helper and a test:
`MODULE.bazel`, `.bazelversion`, `.bazelrc`, `.bazelignore`, `.npmrc`,
`BUILD.bazel`, `package.json`, `pnpm-lock.yaml`, `pnpm-workspace.yaml`,
sources, and `README.md`. The only npm dependency is the framework package, pinned to the version
rules_dx qualifies.
Node comes from the rules_dx toolchain, so nothing needs to be installed.

```sh
bazel run @rules_dx//:dx -- new --apply astro demo
```

Build and test inside the new project:

```text
bazel build //...
bazel test //...
```

Output: text only.

Exit codes: `0` success, `2` usage errors including an invalid
destination or package identity, `1` missing files in check mode or
scaffolding failed.

Scaffolding into an existing directory adds absent files only and leaves
custom files untouched.

## `dx new rust-web`

```text
dx new [--apply] rust-web [name]
```

Scaffolds a standalone shared-Rust consumer: one core library, one native
binary, one unit test suite, one web bundle, and one browser test.
Versions come from the rules_dx pins. Never overwrites existing files.

```sh
bazel run @rules_dx//:dx -- new --apply rust-web demo
```

Build and test inside the new project:

```text
bazel build //...
bazel test //...
```

Output: text only.

Exit codes: `0` success, `2` usage errors including an invalid
destination or package identity, `1` missing files in check mode or
scaffolding failed.

`bazel test //...` runs the browser test in a browser and driver that
Bazel fetches, so nothing needs to be installed by hand. The test is
qualified on linux x86_64, where the host needs GTK 3; other hosts skip
it as incompatible for now. Scaffolding into an existing directory adds absent files
only and leaves custom files untouched.

## `dx upgrade`

```text
dx upgrade [--apply] --from <version> --to <version>
```

Runs the pin, migrate, env, and codegen steps in one go with a recovery pointer. A failed step prints the commands to rerun or undo it, with your `--workspace` and `--apply` choices kept. Checks by
default; `--apply` upgrades. Both
versions are required.
No manifests exist yet, so check and apply fail closed and change nothing.
Flags: `--from <version> --to <version>`, `--apply`.
Scopes: none.

A failure prints `dx: upgrade_failed: <message>` on stderr. With
`--output=json`, an `upgrade_failed` error event follows `command_started`,
then `command_finished`.

Output: `--output text|json`.

Exit codes: `0` success, `2` usage errors including missing versions or a
target that is not newer, `1` a step failed.

## `dx rollback`

```text
dx rollback [--apply]
```

Restores the previous recorded pin in `.dx/version`. Checks by default;
`--apply` restores the last recorded pin, or removes `.dx/version` when no
earlier pin was recorded. The check reports the pin it would restore and
exits `1` until you apply. It runs even when the pin and `MODULE.bazel`
disagree. Refuses without a record, after manual pin edits, or after the
record is deleted. A second `--apply` undoes the rollback.
Flags: `--apply`.
Scopes: none.

A pending rollback fails with `status_pin_mismatch` on stderr and exits `1`:

```text
dx: status_pin_mismatch: pin "<v>" can roll back to "<previous>" (run `dx rollback --apply`)
dx: status_pin_mismatch: pin "<v>" has no earlier pin (run `dx rollback --apply` to remove it)
```

Refusals use the same `status_pin_mismatch` code. When the recorded previous
pin is the current pin, the check prints `pin current: rollback <v>` and exits
`0`. `--apply` prints one of:

```text
pinned <v> (rollback)
removed pin <v> (rollback)
```

After restoring a pin, `--apply` rewrites each existing dx-written launcher
(`tools/dx`, `tools/dx.cmd`) whose version differs, and prints
`wrote <path>` for each one. It never creates a launcher, never touches one
without a `VERSION` line, and leaves launchers alone when it removes the pin.
A previous pin that is not semver refuses before anything is written when a
launcher would change. If the refresh fails, the pin and its record stay
restored and it refuses with:

```text
dx: status_pin_mismatch: pinned <v> but launchers not refreshed: <error> (run `bazel run @rules_dx//:dx -- init --apply`)
```

With `--output=json`, dx streams `command_started`, one `status` event named
`pin` (status `ok`, detail the pin, hint `dx rollback`), then
`command_finished`. Refusals and pending rollbacks emit a
`status_pin_mismatch` error event instead of the `pin` event.

Output: `--output text|json`.

Exit codes: `0` success, `2` usage errors, `1` the pin can roll back or the
rollback was refused.

```sh
bazel run @rules_dx//:dx -- rollback
bazel run @rules_dx//:dx -- rollback --apply
```
