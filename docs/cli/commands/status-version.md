# `dx status` And `dx version`

```sh
bazel run //cli/cli:dx -- status
bazel run //cli/cli:dx -- version
```

## `dx status`

```text
dx status
```

Reports toolchain, platform, tools, and pin drift. Takes no scopes and no
per-command flags. Output is text or JSON. There is no `dx doctor`. Use
`dx status` instead.

Exit codes: `0` all checks pass, `2` usage error, `1` a check failed.

## Failure Explainer

`dx status` prints one line per check:

```text
toolchain: ok (rust 1.98.0 via rules_rust 0.74.0 (MODULE.bazel)) hint: bazel build //...
```

The four checks are `toolchain`, `platform`, `tools`, and `pin`. `pin`
compares `.dx/version` with the `MODULE.bazel` pin. `ok` passes. `error`
fails, and the hint is the fix, for example `dx version --pin <version>`.

`--output=json` streams `command_started`, one `status` event per check with
`name`, `status`, `detail`, and `hint`, then `command_finished`. A failed
check adds a `status_pin_mismatch` error event and exit code `1`. A missing
or empty `.dx/version` fails the same way.

## `dx version`

```text
dx version [--check] [--pin <version>|--rollback]
```

Prints the version.

- `--check`: verify the pin without changing it.
- `--pin <version>`: re-pin to this version.
- `--rollback`: restore the last pin. Conflicts with `--pin`.

Exit codes: `0` success, `2` usage errors including conflicting flags, `1`
pin drift or a refused pin.

```sh
bazel run //cli/cli:dx -- version --check
bazel run //cli/cli:dx -- version --pin 0.0.0
bazel run //cli/cli:dx -- version --rollback
```
