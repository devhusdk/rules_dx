# `dx upgrade`

Moves `rules_dx` and the pin in `.dx/pins` to a version, newer or older. Checks by default; `--apply` writes.

```sh
tools/dx upgrade --to 2.0.0
tools/dx upgrade --to 2.0.0 --apply
tools/dx upgrade --to 1.2.0 --apply
tools/dx upgrade --to 2.0.0 --apply --output json
```

```text
dx upgrade [--apply] --to <version>
```

| Flag | What it does |
| --- | --- |
| `--to <version>` | Version to pin. Required. |
| `--apply` | Writes the changes instead of showing them. |
| `--output text\|json` | Sets the output format. |

Scopes: none. Positional scopes are rejected.

## What `--apply` Writes

1. The `rules_dx` `bazel_dep` version in the root `MODULE.bazel`. It must be one line, `bazel_dep(name = "rules_dx", version = "...")`, the form the launchers read. A dep in an `include()`d file or spread over several lines is refused and nothing is written.
2. `MODULE.bazel.lock`, through `bazel mod deps --lockfile_mode=update`. Skipped when absent.
3. When `--to` differs from this `dx`, the `--to` binary runs `bazel run @rules_dx//:dx -- upgrade --to <to> --apply` and writes the rest. This `dx` returns its exit code.
4. `.dx/pins`, on every run.
5. Existing launchers `tools/dx` and `tools/dx.cmd`.
6. Dx-owned keys of an existing `.vscode/settings.json` and the `uses:` pin in an existing `.github/workflows/ci.yml`. The caller carries no version input.
7. The dx block of an existing `AGENTS.md`.
8. The `third_party/rules_dx/` overrides.
9. Stale preset layers under `.dx/`. `.bazelrc` is untouched.
10. Missing dx entries in existing hooks files, and the `AGENTS.md` import in an existing `CLAUDE.md`.
11. Editor files in `.dx/local/ide/`. A failed refresh prints one `ide_refresh_failed` warning and keeps the exit code.

Steps 1 and 2 are skipped when `rules_dx` is already at `--to`. No step creates a launcher, `AGENTS.md`, `CLAUDE.md`, `.vscode/settings.json` or `ci.yml`.

## Output

The check prints `upgrade current: <to>` and exits `0` when the pin and `rules_dx` are at `--to`. Otherwise it changes nothing and exits `1`. If `MODULE.bazel` would change, it first prints:

```text
MODULE.bazel: rules_dx <old> -> <to>
.dx/pins: <pin> -> <to>
then: bazel mod deps --lockfile_mode=update
```

The `.dx/pins` line appears only when the pin moves. After that come the files a refresh would rewrite:

```text
<path>: refresh
<path>: refreshed by rules_dx <to>
note: <entry>: fix by hand
```

`<path>: refresh` names a stale file when this binary is `<to>`. When `--to` is another version, that version's binary writes the files and this one cannot compare them, so it lists every existing file the refresh may rewrite as `refreshed by rules_dx <to>`, and the check stays pending. `note:` lines (for example a `.bazelignore` without `.dx`) are files refresh never rewrites; they do not keep the check pending.

Then:

```text
dx: upgrade_pending: pin <pin> can move to <to> (run `dx upgrade --to <to> --apply`)
```

`--apply` prints `pinned <to> (upgrade from <pin>)`, or `pinned <to> (upgrade current)` on a rerun, then `wrote <path>` for each changed file. During a handoff it prints only `wrote <path>` lines. Then one line per tool whose base file under `.dx/config/` changed:

```text
<tool>: base changed|added|removed (.dx/config/<dir>/base.<ext>)
```

When a family now selects another tool for a capability, stderr gets:

```text
dx: warning: <family> <capability>: <old> replaced by <new>; rule IDs <old>/<code> become <new>/<code>, migrate native inline suppressions
```

A root file made by `dx config override <tool>` gets the base change through a three-way merge of your file, the old base named in its `dx: override of base <sha>` comment, and the new base. The comment then names the new base. A conflict leaves `<<<<<<< your file` markers, prints `<file>: merge conflict, resolve the markers` and fails with `upgrade_failed` after every other write. When the old base is neither the pre-upgrade base nor in git history, stderr gets `<file>: base <sha> not in history; compare with dx config diff <tool>` and the file is left alone.

Refusals write nothing and emit `upgrade_failed`: no single `bazel_dep(name = "rules_dx", ...)` with a literal version, or a launcher that cannot be refreshed. If the lockfile refresh fails, `MODULE.bazel` and `MODULE.bazel.lock` are restored. If the `--to` binary cannot start, the error names the command to finish by hand.

A failure after the pin lands keeps the pin:

```text
dx: upgrade_failed: pinned <to> but <step> not refreshed: <error>; recovery: rerun `dx upgrade --to <to> --apply` (idempotent)
```

After a move, run `dx init` and `dx sync` when an override file is missing and `MODULE.bazel` does not include the overrides.

JSON streams `command_started`, one `status` event named `pin` (status `ok`, hint `dx status`), then `command_finished`. A refusal emits `upgrade_failed` instead of `pin`. A pending upgrade emits `upgrade_pending`.

## Exit Codes

| Code | Meaning |
| --- | --- |
| `0` | Moved, or already current. |
| `1` | The pin can move (check), or the move was refused. |
| `2` | Usage error, including a missing `--to`. |
