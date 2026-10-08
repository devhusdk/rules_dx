# `dx new` And `dx upgrade`

```sh
bazel run @rules_dx//:dx -- new rust my_project
bazel run @rules_dx//:dx -- upgrade --from 1.0.0 --to 2.0.0
```

## `dx new`

```text
dx new <language> [name]
```

Scaffolds a minimal project for one language: `rust`, `python`,
`javascript`, `typescript`, `go`, `java`, `kotlin`, `scala`, `csharp`,
`fsharp`, `c`, `cc`, or `cpp`. Never overwrites existing files. There is no
`--force`.

Two spellings scaffold the same projects:

- `c#` scaffolds `csharp`.
- `f#` scaffolds `fsharp`.

Output: `--output text`.

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
language rule. `rust` takes Cargo names. `python` takes dotted names.
`javascript` takes npm names. `typescript` takes npm names. `go` takes
module paths. `java` takes Maven artifact ids. `kotlin` takes Maven artifact
ids. `scala` takes Maven artifact ids. `csharp` takes NuGet ids. `fsharp`
takes NuGet ids. `c` carries no package identity. `cc` carries no package
identity. `cpp` carries no package identity. Rejected names fail before any
file is written.

## `dx upgrade`

```text
dx upgrade --from <version> --to <version> [--dry-run]
```

Runs pin, migrate, and setup in one go with a recovery pointer. Both
versions are required. `--dry-run` prints the plan without writing files.
No manifests exist yet, so live runs fail closed.

Output: `--output text|json`.

Exit codes: `0` success, `2` usage errors including missing versions or a
target that is not newer, `1` a step failed.
