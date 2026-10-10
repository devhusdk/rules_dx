# `dx migrate`

```text
dx migrate [--check] [--apply] --from <version> --to <version> [scope...] [--dry-run]
```

Rewrites breaking changes between releases. Bare runs check by default and
write nothing; `--apply` authorizes the migration. Both versions are Cargo
semver. Target must be newer than source.

No manifests exist yet, so every mode fails closed naming the missing
manifest: check, `--apply`, and `--dry-run` all exit `1` instead of printing
a runnable plan. Scopes default to `//...`.

Output: `--output text|json`.

Exit codes: `0` success, `2` usage errors including missing versions or a
target that is not newer, `1` the run failed.

```sh
bazel run @rules_dx//:dx -- migrate --from 1.0.0 --to 2.0.0 --dry-run
```
