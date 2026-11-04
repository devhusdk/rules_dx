# `dx upgrade`

```sh
bazel run @rules_dx//:dx -- upgrade --to 2.0.0
bazel run @rules_dx//:dx -- upgrade --to 2.0.0 --apply
```

```text
dx upgrade [--apply] --to <version>
```

Moves `rules_dx` and the pin in `.dx/version` to `--to`. It reads the current
version from the `rules_dx` `bazel_dep` in `MODULE.bazel`. `--to` may be newer
or older than that version, so the same
command upgrades and rolls back. Checks by default; `--apply` makes the move.
`dx upgrade` runs even when the pin and `MODULE.bazel` disagree, so it is the
fix that version skew names.
Flags: `--to <version>` (required), `--apply`.
Scopes: none.

`--apply` writes, in order:

1. The `bazel_dep(name = "rules_dx", ...)` version in `MODULE.bazel` (or the
   file it `include()`s that holds it), set to `--to`.
2. `MODULE.bazel.lock`, refreshed with
   `bazel mod deps --lockfile_mode=update`. Skipped when the file is absent.
3. When `--to` is not the version of this `dx`, the `--to` binary takes over
   with `bazel run @rules_dx//:dx -- upgrade --to <to> --apply` and writes
   the rest. This `dx` stops there and returns its exit code.
4. `.dx/version` with `--to` and its sha256 lines. It is written on every
   run, so a missing or stale file is regenerated.
5. Each existing dx-written launcher (`tools/dx`, `tools/dx.cmd`) that
   differs from the current text. It never creates a launcher.
6. The dx-owned keys of an existing `.vscode/settings.json` and the pin of
   an existing `.github/workflows/ci.yml`. It never
   creates either file.
7. The dx block of an existing `AGENTS.md`. It never creates the file.
8. The `third_party/rules_dx/` overrides: the whole set (missing files
   created, retired patches removed) when `MODULE.bazel` includes it,
   otherwise only when every generated file already exists.
9. Stale imported preset layers under `.dx/`, without touching `.bazelrc`.
10. Missing dx entries in existing hooks files, and the `AGENTS.md` import
    in an existing `CLAUDE.md`. It never creates either file.

Steps 1 and 2 are skipped when `rules_dx` is already at `--to`.

It refuses with `upgrade_failed` and writes nothing when
`MODULE.bazel` has no single
`bazel_dep(name = "rules_dx", ...)` with a literal version, or when a launcher
could not be refreshed. If the lockfile refresh fails, `MODULE.bazel` and
`MODULE.bazel.lock` are put back as they were and nothing else is written. If
the `--to` binary cannot start, the error names
`bazel run @rules_dx//:dx -- upgrade --to <to> --apply` to finish by hand. A
pin already at `--to` is a rerun: it refreshes the launchers and the block.

The check prints `upgrade current: <to>` and exits `0` when the pin is
already `--to` and `rules_dx` in `MODULE.bazel` is at `--to`. Otherwise it
changes nothing and exits `1`. When `MODULE.bazel` changes it first prints:

```text
MODULE.bazel: rules_dx <old> -> <to>
then: bazel mod deps --lockfile_mode=update
```

Then it prints:

```text
dx: status_pin_mismatch: pin <pin> can move to <to> (run `dx upgrade --to <to> --apply`)
```

When `--to` is another version, `--apply` prints `wrote <path>` for the files
it changed before the handoff. Otherwise it prints `pinned <to> (upgrade from <pin>)`, or
`pinned <to> (upgrade current)` on a rerun, then `wrote <path>` for each
file it changed. A failure after the pin lands keeps the pin and prints the
recovery:

```text
dx: upgrade_failed: pinned <to> but <step> not refreshed: <error>; recovery: rerun `dx upgrade --to <to> --apply` (idempotent)
```

After a move, run `dx init` and `dx env --apply` when an override file is
missing and `MODULE.bazel` does not include the overrides, so the generated `third_party/rules_dx/` overrides match the new pin.

With `--output=json`, dx streams `command_started`, one `status` event named
`pin` (status `ok`, detail the pin, hint `dx status`), then
`command_finished`. A refusal emits an `upgrade_failed` error event instead
of the `pin` event, and a pending check emits `status_pin_mismatch`.

Output: `--output text|json`.

`--apply` also refreshes the editor files in `.dx/ide/`. A failed refresh prints one `ide_refresh_failed` warning and never changes the exit code.

Exit codes: `0` success (moved or already current), `2` usage errors
including a missing `--to`, `1` the pin can move or the move was refused.

```sh
bazel run @rules_dx//:dx -- upgrade --to 1.3.0
bazel run @rules_dx//:dx -- upgrade --to 1.3.0 --apply
bazel run @rules_dx//:dx -- upgrade --to 1.2.0 --apply
```
