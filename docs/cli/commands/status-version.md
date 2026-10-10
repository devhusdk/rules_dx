# `dx status` And `dx version`

```sh
bazel run @rules_dx//:dx -- status
bazel run @rules_dx//:dx -- version
```

## `dx status`

```text
dx status
```

Reports toolchain, platform, tools, and pin drift. Takes no scopes and no
per-command flags. There is no `dx doctor`. Use `dx status` instead.

Output: `--output text|json`.

Exit codes: `0` all checks pass, `2` usage error, `1` a check failed.

## Failure Explainer

`dx status` prints one line per check:

```text
toolchain: ok (rust 1.98.0 via rules_rust 0.74.0 (MODULE.bazel)) hint: bazel build //...
```

The four checks are `toolchain`, `platform`, `tools`, and `pin`. `pin`
compares `.dx/version` with the `MODULE.bazel` pin. `ok` passes. `error`
fails, and the hint is the fix, for example `dx version --pin <version>`.
The `platform` check names the execution platforms tools resolve for. The
target platform never selects tools.

`--output=json` streams `command_started`, one `status` event per check with
`name`, `status`, `detail`, and `hint`, then `command_finished`. A failed
check adds a `status_pin_mismatch` error event and exit code `1`. A missing
or empty `.dx/version` fails the same way.

## `dx version`

```text
dx version [--check] [--apply] [--pin <version>|--rollback]
```

Prints the delivered binary version, the resolved module version with its
source, and the observed pin. Checks by default; `--apply` authorizes `--pin`
and `--rollback`.

- `--check`: verify the pin without changing it.
- `--pin <version>` with `--apply`: re-pin to this version. The pin must
  equal the resolved module version. A successful re-pin records the previous
  pin for rollback.
- `--rollback` with `--apply`: restore the recorded previous pin. Conflicts
  with `--pin`. Refuses without a record, after intervening edits, or after
  the record is deleted.
Flags: `--check`, `--apply`, `--pin`, `--rollback`, `--dry-run`.
Scopes: none.

The module version resolves from the workspace MODULE.bazel dependency and
overrides, falling back to the binary when no MODULE.bazel entry exists.
Overrides are disclosed as their source and never reported as registry
releases. A module that disagrees with the binary refuses `--apply` before
writing. The pin in `.dx/version` is observed rebuildable state; the
MODULE.bazel entry stays authoritative. Deleting
`.dx/version-recovery.json` removes rollback availability.

Output: `--output text|json`.

Exit codes: `0` success, `2` usage errors including conflicting flags, `1`
pin drift, a refused pin, a missing record, or module incompatibility.

```sh
bazel run @rules_dx//:dx -- version --check
bazel run @rules_dx//:dx -- version --apply --pin 0.0.0
bazel run @rules_dx//:dx -- version --apply --rollback
```

## Version Skew

Every command reads the pin in `.dx/version` and compares it with the
`MODULE.bazel` pin before it runs. A mismatch is version skew.

- Runs anyway: `version`, `status`, `completion`, `capabilities`.
- Warns and runs: `check`, `security`, `license`, `owners`, `deps`, `why`, `verify`, `rerun`, `tests`.
- Stops with exit code `1`: every other command.

`--dry-run` turns a stop into a warning. `watch` answers for the command it
wraps. A missing or empty `.dx/version` is not skew, so only the `pin` check
in `dx status` fails on it.

A stop prints `dx: version skew: binary <version> pin <pin> module <version>`
on stderr and names the fix. Under `--output=json` it adds a `version_skew`
error event, then `command_finished` with `exit_code` `1` and
`results_complete` `false`.

```sh
bazel run @rules_dx//:dx -- version --check
bazel run @rules_dx//:dx -- version --apply --pin 0.0.0
bazel run @rules_dx//:dx -- build //...
```
