# `dx upgrade`

```sh
bazel run @rules_dx//:dx -- upgrade --to 2.0.0
bazel run @rules_dx//:dx -- upgrade --to 2.0.0 --apply
```

```text
dx upgrade [--apply] --to <version>
```

Moves the pin in `.dx/version` to `--to`. It reads the current pin from
`.dx/version`. `--to` may be newer or older than the pin, so the same command
upgrades and rolls back. Checks by default; `--apply` moves the pin.
`--to` must be the version of this `dx`: change `rules_dx` in `MODULE.bazel`
first, then run `bazel run @rules_dx//:dx -- upgrade --to <version>`.
`dx upgrade` runs even when the pin and `MODULE.bazel` disagree, so it is the
fix that version skew names.
Flags: `--to <version>` (required), `--apply`.
Scopes: none.

`--apply` writes, in order:

1. `.dx/version` with `--to` and its sha256 lines.
2. Each existing dx-written launcher (`tools/dx`, `tools/dx.cmd`) that
   differs from the current text. It never creates a launcher.
3. Stale generated `.dx/bazelrc` layers.
4. The dx block of an existing `AGENTS.md`. It never creates the file.

It refuses with `upgrade_failed` and writes nothing when `--to` is not the
version of this `dx`, when `.dx/version` is missing or empty (run
`dx init --apply`), or when a launcher could not be refreshed. A pin already
at `--to` is a rerun: it refreshes the launchers, the preset layers and the
block.

The check prints `upgrade current: <to>` and exits `0` when the pin is
already `--to`. Otherwise it changes nothing and exits `1`:

```text
dx: status_pin_mismatch: pin <pin> can move to <to> (run `dx upgrade --to <to> --apply`)
```

`--apply` prints `pinned <to> (upgrade from <pin>)`, or
`pinned <to> (upgrade current)` on a rerun, then `wrote <path>` for each
file it changed. A failure after the pin lands keeps the pin and prints the
recovery:

```text
dx: upgrade_failed: pinned <to> but <step> not refreshed: <error>; recovery: rerun `dx upgrade --to <to> --apply` (idempotent)
```

After a move, run `dx init`, `dx env`, and `dx codegen` so the generated
`third_party/rules_dx/` overrides match the new pin.

With `--output=json`, dx streams `command_started`, one `status` event named
`pin` (status `ok`, detail the pin, hint `dx status`), then
`command_finished`. A refusal emits an `upgrade_failed` error event instead
of the `pin` event, and a pending check emits `status_pin_mismatch`.

Output: `--output text|json`.

Exit codes: `0` success (moved or already current), `2` usage errors
including a missing `--to`, `1` the pin can move or the move was refused.

```sh
bazel run @rules_dx//:dx -- upgrade --to 1.3.0
bazel run @rules_dx//:dx -- upgrade --to 1.3.0 --apply
bazel run @rules_dx//:dx -- upgrade --to 1.2.0 --apply
```
