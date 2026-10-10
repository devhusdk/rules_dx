# `dx new` And `dx upgrade`

```sh
bazel run @rules_dx//:dx -- new rust my_project --check
bazel run @rules_dx//:dx -- upgrade --from 1.0.0 --to 2.0.0 --check
```

## `dx new`

```text
dx new [--check] [--apply] <language> [name]
```

Scaffolds a minimal project for one language: `rust`, `python`,
`javascript`, `typescript`, `go`, `java`, `kotlin`, `scala`, `csharp`,
`fsharp`, `c`, `cc`, `cpp`, or `rust-web`. Checks by default and writes
nothing; `--apply` authorizes the scaffolding. Never overwrites existing
files. A missing project file is drift: the check fails and names every
file `--apply` would write. Files that already exist stay untouched either
way.
There is no `--force`. Runs outside a workspace.

Two spellings scaffold the same projects:

- `c#` scaffolds `csharp`.
- `f#` scaffolds `fsharp`.

Output: `--output text`.

Exit codes: `0` success, `2` usage errors including an unknown language, an
invalid destination or package identity, or extra positionals, `1`
scaffolding failed or the check found drift.

```sh
bazel run @rules_dx//:dx -- new rust my_project --check
bazel run @rules_dx//:dx -- new rust my_project --apply
```

The `[name]` is the destination directory and defaults to `my_project`.
Destinations stay inside the workspace. Absolute paths, `..` segments, empty
segments, backslashes, control characters, and the characters `< > : " | ? *`
are rejected. Segments never end with a space or `.`, and reserved device
names such as `CON` and `NUL` are rejected. Nested destinations such as
`teams/demo` are allowed. Spaces and Unicode are allowed in the destination.

The package identity derives from the final destination segment: it is
lowercased, and every other run of characters folds to `-`. `My App`
scaffolds the `my-app` package. The derived identity must satisfy the
language rule. `rust` takes Cargo names. `rust-web` takes Cargo names and
scaffolds a shared core with native and browser targets: `Cargo.toml`,
`src/lib.rs`, `src/main.rs`, `BUILD.bazel`, `MODULE.bazel`, `.bazelversion`,
and `README.md`. `python` takes dotted names.
`javascript` takes npm names. `typescript` takes npm names. `go` takes
module paths. `java` takes Maven artifact ids. `kotlin` takes Maven artifact
ids. `scala` takes Maven artifact ids. `csharp` takes NuGet ids. `fsharp`
takes NuGet ids. `c` carries no package identity. `cc` carries no package
identity. `cpp` carries no package identity. Rejected names fail before any
file is written.

## `dx new rust-web`

```text
dx new [--check] [--apply] rust-web [name]
```

Scaffolds a standalone shared-Rust consumer: one core library, one native
binary, one unit test suite, one web bundle, and one manual browser test.
Versions come from the rules_dx pins. Never overwrites existing files.

```sh
bazel run @rules_dx//:dx -- new rust-web demo --check
bazel run @rules_dx//:dx -- new rust-web demo --apply
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

Output: `--output text`.

Exit codes: `0` success, `2` usage errors including an invalid
destination or package identity, `1` scaffolding failed.

The browser test is manual. It needs Firefox and geckodriver on a
qualified host. Scaffolding into an existing directory adds absent files
only and leaves custom files untouched.

## `dx upgrade`

```text
dx upgrade [--check] [--apply] --from <version> --to <version> [--dry-run]
```

Runs pin, migrate, and setup in one go with a recovery pointer. Checks by
default and writes nothing; `--apply` authorizes the upgrade. Both
versions are required. `--dry-run` prints the plan without writing files.
No manifests exist yet, so live runs fail closed in every mode: the check
validates the pin, migrate, and setup preconditions without manufacturing a
plan, and `--apply` reports the same missing-manifest failure.

Output: `--output text|json`.

Exit codes: `0` success, `2` usage errors including missing versions or a
target that is not newer, `1` a step failed.

```sh
bazel run @rules_dx//:dx -- upgrade --from 1.0.0 --to 2.0.0 --check
bazel run @rules_dx//:dx -- upgrade --from 1.0.0 --to 2.0.0 --dry-run
```
