# `dx migrate`

```text
dx migrate --from <version> --to <version> [scope...] [--dry-run]
```

Rewrites breaking changes between releases. Both versions are Cargo semver.
Target must be newer than source. `--dry-run` prints the plan without
writing files.

No manifests exist yet, so live runs fail closed. Scopes default to `//...`.

```sh
bazel run //cli/cli:dx -- migrate --from 1.0.0 --to 2.0.0 --dry-run
```
