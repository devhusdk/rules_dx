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
`fsharp`, `c`, `cpp`, `rust-web`, `ruby`, `powershell`, `astro`, `svelte`, `vue`, `mdx`, `zig`, `elixir`, or `gleam`. Never overwrites existing files.
There is no `--force`. Runs outside a workspace.

`c` scaffolds C sources and `cpp` scaffolds C++ sources.

Removed spellings fail with exit `2` and name the replacement:

- `cc` is now `cpp`.
- `c#` and `c-sharp` are now `csharp`.
- `f#` and `f-sharp` are now `fsharp`.

Output: text only.

Exit codes: `0` success, `2` usage errors including an unknown language, an
invalid destination or package identity, or extra positionals, `1`
scaffolding failed.

The `[name]` is the destination directory and defaults to `my_project`.
Destinations stay inside the workspace. Absolute paths, `..` segments, empty
segments, backslashes, control characters, and the characters `< > : " | ? *`
are rejected. Segments never end with a space or `.`, and reserved device
names such as `CON` and `NUL` are rejected. Nested destinations such as
`teams/demo` are allowed. Spaces and Unicode are allowed in the destination.

The package identity derives from the final destination segment: it is
lowercased, and every other run of characters folds to `-`. `My App`
scaffolds the `my-app` package. The derived identity must satisfy the
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
- `c` carries no package identity.
- `cpp` carries no package identity.
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

`rust-web` scaffolds a shared core with native and browser targets:
`Cargo.toml`, `src/lib.rs`, `src/main.rs`, `BUILD.bazel`, `MODULE.bazel`,
`.bazelversion`, and `README.md`. A Bazel module name uses lowercase letters,
digits, dots, hyphens and underscores, starts with a letter, and ends with a
letter or digit. Rejected names fail before any
file is written.

## `dx new ruby`, `dx new powershell`, `dx new zig`, `dx new elixir` And `dx new gleam`

```text
dx new [--apply] ruby [name]
dx new [--apply] powershell [name]
dx new [--apply] zig [name]
dx new [--apply] elixir [name]
dx new [--apply] gleam [name]
```

Scaffolds a standalone consumer with a library, a binary and a test:
`MODULE.bazel`, `.bazelversion`, `.bazelrc`, `BUILD.bazel`, sources, and
`README.md`. Ruby, PowerShell, Zig, Elixir and Gleam come from the rules_dx toolchains, so nothing needs to be
installed. The PowerShell targets are Linux only. The Elixir and Gleam targets need Linux x86_64. The Zig library imports as `hello`.
The Elixir starter is the OTP application `hello` with sources in `lib/`, an
escript, and an ExUnit test in `test/`.
The Gleam starter has the module `hello` in `src/`, a gleeunit test in `test/`,
and a binary, library and test for both Erlang and JavaScript. The JavaScript
targets end in `_js` and run on Node.js.

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
destination or package identity, `1` scaffolding failed.

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
destination or package identity, `1` scaffolding failed.

Scaffolding into an existing directory adds absent files only and leaves
custom files untouched.

## `dx new rust-web`

```text
dx new [--apply] rust-web [name]
```

Scaffolds a standalone shared-Rust consumer: one core library, one native
binary, one unit test suite, one web bundle, and one manual browser test.
Versions come from the rules_dx pins. Never overwrites existing files.

```sh
bazel run @rules_dx//:dx -- new --apply rust-web demo
```

Build and test inside the new project:

```text
bazel build //...
bazel test //...
```

Run the browser test with a local driver:

```text
geckodriver --port 4444 &
bazel test :demo_browser_test --test_env=GECKODRIVER_REMOTE=http://127.0.0.1:4444
```

Output: text only.

Exit codes: `0` success, `2` usage errors including an invalid
destination or package identity, `1` scaffolding failed.

The browser test is manual. It needs Firefox and geckodriver on a
qualified host. Scaffolding into an existing directory adds absent files
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

Output: `--output text|json`.

Exit codes: `0` success, `2` usage errors including missing versions or a
target that is not newer, `1` a step failed.

## `dx rollback`

```text
dx rollback [--apply]
```

Restores the previous recorded pin in `.dx/version`. Checks by default;
`--apply` restores. The check reports the pin it would restore and exits `1`
until you apply. It runs even when the pin and `MODULE.bazel` disagree.
Refuses without a record, after manual pin edits, or after the record is
deleted.
Flags: `--apply`.
Scopes: none.

Output: `--output text|json`.

Exit codes: `0` success, `2` usage errors, `1` the pin can roll back or the
rollback was refused.

```sh
bazel run @rules_dx//:dx -- rollback
bazel run @rules_dx//:dx -- rollback --apply
```
