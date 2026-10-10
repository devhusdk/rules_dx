# `dx migrate`

```text
dx migrate [--apply] --from <version> --to <version> [scope...] [--dry-run]
```

Rewrites breaking changes between releases. Checks by default; `--apply`
migrates. Both versions are Cargo semver.
Target must be newer than source. `--dry-run` validates the manifest
without writing files.

No manifests exist yet, so dry-run, check, and apply fail closed. Scopes default to `//...`.
Flags: `--from <version> --to <version>`, `--apply`, `--dry-run`.
Scopes: explicit Bazel labels/patterns or workspace-relative files/dirs.

Output: `--output text|json`.

Exit codes: `0` success, `2` usage errors including missing versions or a
target that is not newer, `1` the run failed.

```sh
bazel run @rules_dx//:dx -- migrate --from 1.0.0 --to 2.0.0 --dry-run
```
