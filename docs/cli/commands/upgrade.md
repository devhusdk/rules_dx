# `dx upgrade`

Use `dx upgrade` to move your workspace to another rules_dx version, newer or older. It checks by default; `--apply` writes.

It moves the `rules_dx` dependency in `MODULE.bazel` and the pin in `.dx/pins`, then refreshes the existing launchers and the `AGENTS.md` block.

```sh
tools/dx upgrade
tools/dx upgrade --to 2.0.0
tools/dx upgrade --to 2.0.0 --apply
tools/dx upgrade --to 1.2.0 --apply
tools/dx upgrade --to 2.0.0 --apply --output json
```

## Usage

```text
dx upgrade [--apply] [--to <version>]
```

## Flags

| Flag | What it does |
| --- | --- |
| `--to <version>` | Version to pin. Defaults to the newest stable `rules_dx` in the Bazel registry. |
| `--apply` | Write the changes instead of showing them. |
| `-o, --output text\|json` | Choose the output format. |
| `--workspace <dir>` | Use this workspace directory. |
| `-q, --quiet` | Hide summaries. |
| `-v, --verbose` | Show more logs. |
| `--color auto\|always\|never` | Set color output. |
| `--build-slots <auto\|n\|off>` | Set build slots: `auto`, a count, or `off`. |
| `--build-slots-wait <duration>` | Give up waiting for a build slot after this long. `0` fails at once. |
| `--bazel-startup-option <token>` | Pass one Bazel startup option. Repeatable. |

## Scopes

None. A scope is a usage error.

Without `--to`, `dx upgrade` reads `modules/rules_dx/metadata.json` from the registry and uses its newest stable version, so the check prints `pin <old> can move to <new>` with the `--apply` command. If the registry cannot be reached it fails with `upgrade needs --to <version>`.

## What `--apply` writes

1. The `rules_dx` `bazel_dep` version in the root `MODULE.bazel`. It must be one line, `bazel_dep(name = "rules_dx", version = "...")`. A dep in an `include()`d file or spread over several lines is refused and nothing is written.
2. `MODULE.bazel.lock`, through `bazel mod deps --lockfile_mode=update`. Skipped when absent.
3. When `--to` differs from this `dx`, the `--to` binary runs `bazel run @rules_dx//dx/launch:dx -- upgrade --to <to> --apply` and writes the rest. This `dx` returns its exit code.
4. `.dx/pins`, on every run.
5. Existing launchers `tools/dx` and `tools/dx.cmd`.
6. Dx-owned keys of an existing `.vscode/settings.json` and the `uses:` pin in an existing `.github/workflows/ci.yml`. The caller carries no version input.
7. The dx block of an existing `AGENTS.md`.
8. Stale preset layers under `.dx/`. `.bazelrc` is untouched.
9. Missing dx entries in existing hooks files, and the `AGENTS.md` import in an existing `CLAUDE.md`.
10. Editor files in `.dx/local/ide/`. A failed refresh prints one `ide_refresh_failed` warning and keeps the exit code.

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

See [Silence one finding](quality.md#silence-one-finding).

A root file made by `dx config override <tool>` gets the base change through a three-way merge of your file, the old base named in its `dx: override of base <sha>` comment, and the new base. The comment then names the new base. A conflict leaves `<<<<<<< your file` markers, prints `<file>: merge conflict, resolve the markers` and fails with `upgrade_failed` after every other write. When the old base is neither the pre-upgrade base nor in git history, stderr gets `<file>: base <sha> not in history; compare with dx config diff <tool>` and the file is left alone.

Refusals write nothing and emit `upgrade_failed`: no single `bazel_dep(name = "rules_dx", ...)` with a literal version, or a launcher that cannot be refreshed. If the lockfile refresh fails, `MODULE.bazel` and `MODULE.bazel.lock` are restored. If the `--to` binary cannot start, the error names the command to finish by hand.

A failure after the pin lands keeps the pin:

```text
dx: upgrade_failed: pinned <to> but <step> not refreshed: <error>; recovery: rerun `dx upgrade --to <to> --apply` (idempotent)
```

`dx upgrade` writes no reports.

JSON streams `command_started`, one `status` event named `pin` (severity `info`, hint `dx status`), then `command_finished`. A refusal emits `upgrade_failed` instead of `pin`. A pending upgrade emits `upgrade_pending`.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Moved, or already current. |
| `1` | The pin can move (check), or the move was refused. |
| `2` | Usage error, including no `--to` when the registry cannot be reached. |
| `3` | A file could not be read or written, or the `--to` binary could not start. |
