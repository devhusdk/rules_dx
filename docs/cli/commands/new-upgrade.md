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

Exit codes: `0` success, `2` usage errors including an unknown language or
extra positionals, `1` scaffolding failed.

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
