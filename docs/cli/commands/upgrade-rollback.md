# `dx upgrade` And `dx rollback`

```sh
bazel run @rules_dx//:dx -- upgrade --from 1.0.0 --to 2.0.0
bazel run @rules_dx//:dx -- rollback
```

## `dx upgrade`

```text
dx upgrade [--apply] --from <version> --to <version>
```

Moves the pin in `.dx/version` from `--from` to `--to`. Checks by default;
`--apply` upgrades. Both versions are required, and `--to` must be newer.
`dx upgrade` runs even when the pin and `MODULE.bazel` disagree, so it is
the fix that version skew names when the pin is older than the module version. After a rollback it names `dx init`, `dx env`, and `dx codegen` so the generated `third_party/rules_dx/` overrides match the restored pin.
Flags: `--from <version> --to <version>`, `--apply`.
Scopes: none.

`--apply` writes, in order:

1. `.dx/version` with `--to`.
2. The pin record `dx rollback` reads, with `--from` as the previous pin.
3. Each existing dx-written launcher (`tools/dx`, `tools/dx.cmd`) whose
   version differs. It never creates a launcher.
4. The dx block of an existing `AGENTS.md`. It never creates the file.

It refuses with `upgrade_failed` and writes nothing when `--to` is not the
version of this `dx` (run the `--to` binary with
`bazel run @rules_dx//:dx -- upgrade ...`), when `.dx/version` is missing or
empty (run `dx init --apply`), when the pin is neither `--from` nor `--to`,
or when a launcher could not be refreshed. A pin already at `--to` is a rerun:
it refreshes the launchers, the block and stale Bazel preset layers and leaves the record alone.

The check prints `upgrade current: <to>` and exits `0` when the pin is
already `--to`. Otherwise it changes nothing and exits `1`:

```text
dx: status_pin_mismatch: pin <from> can upgrade to <to> (run `dx upgrade --from <from> --to <to> --apply`)
```

`--apply` prints `pinned <to> (upgrade from <from>)`, or
`pinned <to> (upgrade current)` on a rerun, then `wrote <path>` for each
launcher or `AGENTS.md` it changed. A failure after the pin lands keeps the
pin and its record, and prints the recovery:

```text
dx: upgrade_failed: pinned <to> but <step> not refreshed: <error>; recovery: rerun `dx upgrade --from <from> --to <to> --apply` (idempotent); ...
```

With `--output=json`, dx streams `command_started`, one `status` event named
`pin` (status `ok`, detail the pin, hint `dx rollback`), then
`command_finished`. A refusal emits an `upgrade_failed` error event instead
of the `pin` event, and a pending check emits `status_pin_mismatch`.

Output: `--output text|json`.

Exit codes: `0` success (upgraded or already current), `2` usage errors
including missing versions or a target that is not newer, `1` the pin can
upgrade or the upgrade was refused.

```sh
bazel run @rules_dx//:dx -- upgrade --from 1.2.0 --to 1.3.0
bazel run @rules_dx//:dx -- upgrade --from 1.2.0 --to 1.3.0 --apply
```

## `dx rollback`

```text
dx rollback [--apply]
```

Restores the previous recorded pin in `.dx/version`. `dx upgrade --apply`
and `dx rollback --apply` write the record. Checks by default;
`--apply` restores the last recorded pin, or removes `.dx/version` when no
earlier pin was recorded. The check reports the pin it would restore and
exits `1` until you apply. It runs even when the pin and `MODULE.bazel`
disagree. Refuses without a record, after manual pin edits, or after the
record is deleted. A second `--apply` undoes the rollback.
Flags: `--apply`.
Scopes: none.

A pending rollback fails with `status_pin_mismatch` on stderr and exits `1`:

```text
dx: status_pin_mismatch: pin "<v>" can roll back to "<previous>" (run `dx rollback --apply`)
dx: status_pin_mismatch: pin "<v>" has no earlier pin (run `dx rollback --apply` to remove it)
```

Refusals use the same `status_pin_mismatch` code. When the recorded previous
pin is the current pin, the check prints `pin current: rollback <v>` and exits
`0`. `--apply` prints one of:

```text
pinned <v> (rollback)
removed pin <v> (rollback)
```

After restoring a pin, `--apply` rewrites each existing dx-written launcher
(`tools/dx`, `tools/dx.cmd`) whose version differs, and prints
`wrote <path>` for each one. It never creates a launcher, never touches one
without a `VERSION` line, and leaves launchers alone when it removes the pin. When the previous pin is this dx it also rewrites stale generated `.dx/bazelrc` layers; for another version it prints a hint to run `tools/dx init --apply` under the restored pin.
A previous pin that is not semver refuses before anything is written when a
launcher would change. If the refresh fails, the pin and its record stay
restored and it refuses with:

```text
dx: status_pin_mismatch: pinned <v> but launchers not refreshed: <error> (run `bazel run @rules_dx//:dx -- init --apply`)
```

With `--output=json`, dx streams `command_started`, one `status` event named
`pin` (status `ok`, detail the pin, hint `dx rollback`), then
`command_finished`. Refusals and pending rollbacks emit a
`status_pin_mismatch` error event instead of the `pin` event.

Output: `--output text|json`.

Exit codes: `0` success, `2` usage errors, `1` the pin can roll back or the
rollback was refused.

```sh
bazel run @rules_dx//:dx -- rollback
bazel run @rules_dx//:dx -- rollback --apply
```
