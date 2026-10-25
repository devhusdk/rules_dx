# `dx migrate`

```text
dx migrate [--apply] --from <version> --to <version> [scope...]
```

Rewrites breaking changes between releases. Checks by default; `--apply`
migrates. Both versions are Cargo semver.
Target must be newer than source. A major `dx bump` names `dx migrate`.

No manifests exist yet, so check and apply fail closed.
Flags: `--from <version> --to <version>`, `--apply`.
`--check`, `--fail-on`, `--report`, `--offline`, `--frozen`, `--here`,
`--affected`, and Bazel options do not apply.
Scopes: explicit Bazel labels/patterns or workspace-relative files/dirs.
No scope selects `//...`. Scopes are accepted but not read until a migration
ships.

Output: `--output text|json`. Text prints the failure on stderr:
`dx: migrate_failed: no qualified migration <from> -> <to>: manifest <manifest> is not shipped`.
The manifest is `migrate-vA-to-vB.json` for a major bump and
`migrate-v<from>-to-v<to>.json` for a minor or patch bump. `-v` first prints
`Would migrate <from> -> <to> via <manifest>` on stdout. `json` emits
`command_started` (mode `check`, or `default` with `--apply`), one `error`
event (`migrate_failed`), and `command_finished`. A usage error prints
`dx: <message>` and the usage line on stderr; `json` adds an `error` event
(`invalid_arguments`) and `command_finished`.

Reports: none. `--report` is a usage error.

Exit codes: `0` success, which no run reaches until a migration ships. `2`
usage errors: missing, empty, or non-semver versions, a target that is not
newer (including equal versions), `--report`, and flags or Bazel options that
do not apply. `1` `migrate_failed`, returned by every valid upgrade today.

```sh
bazel run @rules_dx//:dx -- migrate --from 1.0.0 --to 2.0.0
bazel run @rules_dx//:dx -- migrate --from 1.0.0 --to 2.0.0 --output=json
```
