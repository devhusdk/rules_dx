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

Reports toolchain, Bazel, module, host, platform, tools, pin, and agents
health. With `--verbose` it adds one capability record per `dx` command. Takes no scopes and no
per-command flags. Runs anywhere with a workspace. It is read-only: it
downloads, installs, repairs, and runs nothing. There is no `dx doctor`.
Use `dx status` instead.

Output: `--output text|json`. `--verbose` adds the capability records.

Exit codes: `0` all checks pass, `2` usage error, `1` a check failed.

## Failure Explainer

`dx status` prints one line per check. Inside a `dx worktree` slot it adds a
`lease` check that names the holder and branch:

```text
toolchain: ok (rust 1.98.0 via rules_rust 0.74.0 (MODULE.bazel)) hint: bazel build //...
```

The checks are `toolchain`, `bazel`, `module`, `host`, `platform`, `tools`,
`pin`, and `agents`. Inside a worktree slot there is also `lease`.

- `bazel`: reads `.bazelversion` and looks for `bazel` or `bazelisk` on `PATH`.
- `module`: compares the `rules_dx` version in `MODULE.bazel` with the `dx` version.
- `host`: checks the machine is a v1 execution platform. This is not native qualification.
- `platform`: names the execution platforms tools resolve for. The target
  platform never selects tools.
- `tools`: reads the managed tools `dx env --apply` selected under
  `.dx/setups/current`. `warn` when none is selected. `error` when the
  selection is foreign, its environment or `values.json` is missing or
  corrupt, or a tool link is missing, is not a link, or points at a missing
  Bazel artifact. Pinned quality tools resolve at build time and are not
  probed.
- `pin`: compares `.dx/version` with the `MODULE.bazel` pin.
- `agents`: `warn` when `AGENTS.md` has no dx block or an out-of-date one, or
  `.claude/settings.json` lacks a `dx worktree` hook.
  `dx init --apply` writes it.

`ok` passes. `warn` means unknown and does not fail. `error` fails, and
the hint is the fix, for example `dx upgrade --from <version> --to <version>`.
Detail text starts with `available:`, `unavailable:`, `unknown:`, or
`not applicable:`. Unknown is never healthy.

With `--verbose`, text output then prints one `capability <name>` line per
command, then `capability workspace:tools`.

`--output=json` streams `command_started`, one `status` event per check with
`name`, `status`, `detail`, and `hint`, then `command_finished`. With
`--verbose` it adds one `capability` event per command and one for
`workspace:tools` before `command_finished`. A `capability` event
has `name`, `source`, `availability`, `detail`, `flags`, `outputs`,
`reports`, `scope_policy`, `effect`, and `passthrough`. `source` is
`cli-grammar` for commands and `workspace-record` for `workspace:tools`.
`availability` is `available`, `unavailable`, or `unknown`, and
`workspace:tools` follows the `tools` check. A grammar record says the
command is accepted, not that its tools are installed.

A failed `pin` check adds a `status_pin_mismatch` error event and exit code
`1`. A missing or empty `.dx/version` fails the same way. Any other failed
check adds a `status_check_failed` error event that names the failed checks,
and exit code `1`.

```text
dx: status_check_failed: failed checks: module, tools (see hint)
```

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

- Runs anyway: `version`, `status`, `rollback`, `completion`, `worktree`.
- Warns and runs: `check`, `security`, `license`, `list-tests`.
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
