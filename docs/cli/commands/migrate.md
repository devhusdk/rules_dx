# `dx migrate`

```text
dx migrate [--apply] --from <version> --to <version> [scope...] [--dry-run]
```

Rewrites breaking changes between releases. Both versions are Cargo semver.
Target must be newer than source. `--dry-run` prints the plan without
writing files.

No manifests exist yet, so live runs fail closed. Scopes default to `//...`.

Output: `--output text|json`.

Exit codes: `0` success, `2` usage errors including missing versions or a
target that is not newer, `1` the run failed.

```sh
bazel run @rules_dx//:dx -- migrate --from 1.0.0 --to 2.0.0 --dry-run
```
