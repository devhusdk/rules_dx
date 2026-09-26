# `dx new` And `dx upgrade`

```sh
bazel run //cli/cli:dx -- new rust my_project
bazel run //cli/cli:dx -- upgrade --from 1.0.0 --to 2.0.0
```

## `dx new`

```text
dx new <language> [name]
```

Scaffolds a minimal project for one language: `rust`, `python`,
`javascript`, `typescript`, `go`, `java`, `kotlin`, `scala`, `csharp`,
`fsharp`, `c`, `cc`, or `cpp`. Never overwrites existing files. There is no
`--force`.

## `dx upgrade`

```text
dx upgrade --from <version> --to <version> [--dry-run]
```

Runs pin, migrate, and setup in one go with a recovery pointer. Both
versions are required. `--dry-run` prints the plan without writing files.
No manifests exist yet, so live runs fail closed.
