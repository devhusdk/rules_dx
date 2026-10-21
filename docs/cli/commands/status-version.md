# `dx status` And `dx version`

```sh
bazel run @rules_dx//:dx -- status
bazel run @rules_dx//:dx -- status --output=json
bazel run @rules_dx//:dx -- version
```

## `dx status`

```text
dx status
```

Reports toolchain, platform, tools, and pin drift, then one capability
record per `dx` command. Takes no scopes and no per-command flags. Runs
anywhere with a workspace, downloads nothing, and probes no tools. There is no `dx doctor`. Use `dx status` instead.

Output: `--output text|json`.

Exit codes: `0` all checks pass, `2` usage error, `1` a check failed.

## Failure Explainer

`dx status` prints one line per check:

```text
toolchain: ok (rust 1.98.0 via rules_rust 0.74.0 (MODULE.bazel)) hint: bazel build //...
```

The four checks are `toolchain`, `platform`, `tools`, and `pin`. `pin`
compares `.dx/version` with the `MODULE.bazel` pin. `ok` passes. `error`
fails, and the hint is the fix, for example `dx upgrade --from <version> --to <version>`.
The `platform` check names the execution platforms tools resolve for. The
target platform never selects tools.

After the checks, text output prints one `capability <name>` line per
command, then `capability workspace:tools`.

`--output=json` streams `command_started`, one `status` event per check with
`name`, `status`, `detail`, and `hint`, one `capability` event per command
and one for `workspace:tools`, then `command_finished`. A `capability` event
has `name`, `source`, `availability`, `detail`, `flags`, `outputs`,
`reports`, `scope_policy`, `effect`, and `passthrough`. `source` is
`cli-grammar` or `unobserved`. `availability` is `available`,
`unavailable`, or `unknown`. A grammar record says the command is accepted,
not that its tools are installed. `unknown` means status did not observe it. A failed
check adds a `status_pin_mismatch` error event and exit code `1`. A missing
or empty `.dx/version` fails the same way.

## `dx version`

```text
dx version [--check]
```

Prints the version. Read-only: it never writes the pin. Bare `dx version`
also prints the `rules_dx` dependency the workspace declares, including any
override and its source.

- `--check`: verify the pin without changing it.

To change the pin, run `dx upgrade`. To restore the previous pin, run
`dx rollback`. `dx version --pin` and `dx version --rollback` exit `2` and
name those commands.

Flags: `--check`.
Scopes: none.

Output: `--output text|json`. Bare JSON adds a `dependency` status event.

Exit codes: `0` success, `2` usage errors including removed flags, `1` pin
drift.

```sh
bazel run @rules_dx//:dx -- version
bazel run @rules_dx//:dx -- version --check
```

## Version Skew

Every command reads the pin in `.dx/version` and compares it with the
`MODULE.bazel` pin before it runs. A mismatch is version skew.

- Runs anyway: `version`, `status`, `rollback`, `completion`.
- Warns and runs: `check`, `security`, `license`, `owners`, `deps`, `why`, `tests`.
- Stops with exit code `1`: every other command.

`watch` answers for the command it
wraps. A missing or empty `.dx/version` is not skew, so only the `pin` check
in `dx status` fails on it.

A stop prints `dx: version skew: binary <version> pin <pin> module <version>`
on stderr and names the fix. Under `--output=json` it adds a `version_skew`
error event, then `command_finished` with `exit_code` `1` and
`results_complete` `false`.

```sh
bazel run @rules_dx//:dx -- version --check
bazel run @rules_dx//:dx -- rollback
bazel run @rules_dx//:dx -- build //...
```
