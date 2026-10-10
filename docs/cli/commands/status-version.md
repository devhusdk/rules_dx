# `dx status` And `dx version`

```sh
bazel run @rules_dx//:dx -- status
bazel run @rules_dx//:dx -- version
```

## `dx status`

```text
dx status [--migrate-config] [--apply]
```

Reports toolchain, platform, tools, pin drift, and effective config. Takes
no scopes. There is no `dx doctor`. Use `dx status` instead.

- `--migrate-config`: plan moving legacy `.dx/config.toml` keys into the
  committed `dx.toml`. Check by default, writes nothing.
- `--apply`: authorize the legacy-config migration. Needs
  `--migrate-config`. Without it `dx status` only reports.

Output: `--output text|json`.

Exit codes: `0` all checks pass, `2` usage error, `1` a check failed.

## Failure Explainer

`dx status` prints one line per check:

```text
toolchain: ok (rust 1.98.0 via rules_rust 0.74.0 (MODULE.bazel)) hint: bazel build //...
```

The five checks are `toolchain`, `platform`, `tools`, `pin`, and `config`.
`pin` compares `.dx/version` with the `MODULE.bazel` pin. `ok` passes.
`error` fails, and the hint is the fix, for example
`dx version --pin <version>`. `config` names the config files in play and
the origin of every default: `flag`, `env`, `local` (`dx.local.toml`),
`committed` (`dx.toml`), `legacy` (`.dx/config.toml`), or `built-in`.
A legacy file, or a legacy file next to a new one, reports `warn` with a
`dx status --migrate-config --apply` hint.

`--output=json` streams `command_started`, one `status` event per check with
`name`, `status`, `detail`, and `hint`, then `command_finished`. A failed
check adds a `status_pin_mismatch` error event and exit code `1`. A missing
or empty `.dx/version` fails the same way.

## `dx version`

```text
dx version [--check] [--apply] [--pin <version>|--rollback]
```

Prints the version.

- `--check`: verify the pin without changing it.
- `--pin <version>`: re-pin to this version.
- `--rollback`: restore the last pin. Conflicts with `--pin`.

Output: `--output text|json`.

Exit codes: `0` success, `2` usage errors including conflicting flags, `1`
pin drift or a refused pin.

```sh
bazel run @rules_dx//:dx -- version --check
bazel run @rules_dx//:dx -- version --pin 0.0.0
bazel run @rules_dx//:dx -- version --rollback
```

## Version Skew

Every command reads the pin in `.dx/version` and compares it with the
`MODULE.bazel` pin before it runs. A mismatch is version skew.

- Runs anyway: `version`, `status`, `completion`, `capabilities`.
- Warns and runs: `check`, `security`, `license`, `owners`, `deps`, `why`.
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
bazel run @rules_dx//:dx -- version --pin 0.0.0
bazel run @rules_dx//:dx -- build //...
```
