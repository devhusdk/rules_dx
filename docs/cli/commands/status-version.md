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

Reports toolchain, Bazel, module, overrides, host, exec_tools, tools, pin, and agents
health. With `--verbose` it adds one capability record per `dx` command. Takes no scopes and no
per-command flags. Runs anywhere with a workspace. It is read-only: it
downloads, installs, repairs, and runs nothing.

Output: `--output text|json`. `--verbose` adds the capability records.

Exit codes: `0` all checks pass, `2` usage error, `1` a check failed or
`.dx/version` is missing.

## Failure Explainer

`dx status` prints one line per check, for example:

```text
toolchain: ok (available: rust 1.98.0 via rules_rust 0.74.0 declared in MODULE.bazel) hint: bazel build //...
```

Inside a `dx worktree` slot it adds a `lease` check that names the holder and branch.

The checks are `toolchain`, `bazel`, `module`, `overrides`, `host`, `exec_tools`,
`tools`, `pin`, and `agents`. With a dx launcher there is also `launcher`.
Inside a worktree slot there is also `lease`.

- `toolchain`: reads the rust toolchain versions and the `rules_rust` version
  declared in `MODULE.bazel`. `ok` when they match the versions `dx` is tested
  with. `warn` when `MODULE.bazel` is missing, does not parse, declares no rust
  toolchain, takes `rules_rust` from an override, or declares other versions.
  It never fails `dx status`, and it reports what `MODULE.bazel` declares, not
  what Bazel resolves.
- `bazel`: reads `.bazelversion` and looks for `bazel` or `bazelisk` on `PATH`.
- `module`: compares the `rules_dx` version in `MODULE.bazel`, or in a file it `include()`s, with the `dx` version. It reports `error` with the parser message when either cannot be read or parsed.
  A `single_version_override` version wins over `bazel_dep`. An override
  without a version falls back to the `bazel_dep` version. The check is `warn`
  only when neither declares a version.
- `overrides`: compares the dx-generated `third_party/rules_dx/` files and the
  `include("//third_party/rules_dx:rules_dx.MODULE.bazel")` line with this `dx`.
  `ok` when they match, or `not applicable` in rules_dx itself or without a
  `rules_dx` `bazel_dep`. `warn` when the include is missing, `MODULE.bazel`
  overrides one of those modules itself, or the `rules_dx` pin differs from
  `dx`. `error` when a generated file is missing or changed, or when the include
  and a consumer-owned override clash. The hint is `tools/dx init --apply`.
- `host`: checks the machine is a v1 execution platform: linux_x86_64,
  linux_arm64, macos_arm64, windows_x86_64, or windows_arm64. The target platform never
  selects tools. This is not native qualification.
- `exec_tools`: reads `.bazelrc` and the vendored preset. `warn` when
  `--host_compilation_mode=fastbuild` is not set for `build` or `dx_dev`, so exec tools build at opt.
- `tools`: reads the managed tools `dx env --apply` selected under
  `.dx/setups/current`. `warn` when none is selected. `error` when the
  selection is foreign, its environment or `values.json` is missing or
  corrupt, or a tool link is missing, is not a link, or points at a missing
  Bazel artifact. Pinned quality tools resolve at build time and are not
  probed.
- `pin`: compares `.dx/version` with the rules_dx module version this `dx`
  was built from. An empty or invalid (non-semver) pin hints
  `write <module version> to .dx/version`.
- `launcher`: shown only when `tools/dx` or `tools/dx.cmd` is a dx launcher
  (see `dx init`). `error` when that version differs from `.dx/version`, with detail
  `tools/dx pins dx <v>, .dx/version is <pin>` and hint
  `bazel run @rules_dx//:dx -- init --apply`. It fails `dx status` with
  `status_check_failed`.
- `agents`: `warn` when `AGENTS.md` has no dx block or an out-of-date one, or
  `CLAUDE.md` does not import `AGENTS.md`, or
  `.claude/settings.json` lacks a `dx worktree` hook, or `.codex/hooks.json`
  or `.gemini/settings.json` exists and lacks the guard hook.
  `ok` with `agents part not selected` when `.dx/init.toml` does not list
  `agents` and none of these files exist.
  `dx init --apply --with=agents` writes it.

`ok` passes. `warn` means unknown and does not fail. `error` fails, and
the hint is the fix, for example `dx upgrade --from <version> --to <version> --apply`.
The `toolchain`, `bazel`, `module`, `overrides`, `host`, `exec_tools`, and `tools` detail text starts with
`available:`, `unavailable:`, `unknown:`, or `not applicable:`. Unknown is
never healthy.

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

A failure emits exactly one error event and exits `1`. If `pin` fails, the
event is `status_pin_mismatch`, even when other checks fail too. Otherwise
any failed check adds `status_check_failed`, which names the failed checks.
An empty `.dx/version` fails the `pin` check.

A missing `.dx/version` stops `dx status` with `status_pin_mismatch`
(`read version pin: ...`) before any check runs. It prints no check lines.
JSON output has only `command_started`, `error`, and `command_finished`.

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

To change the pin: when the pin is older than the module version, run
`dx upgrade --from <pin> --to <module version> --apply`. When it is newer or
invalid, write the module version to `.dx/version`. `dx rollback --apply`
restores the last pin `dx upgrade` or `dx rollback` recorded.

Flags: `--check`.
Scopes: none.

Output: `--output text|json`. Bare text prints one line each:
`dx <version>`, `rules_dx <version>`, `pin <pin>`, and
`dependency <dependency>`.

Bare JSON emits `binary`, `module`, `pin`, and `dependency` status events.
`pin` is `warn` when the pin differs from the module version. `dependency` is
`warn` when MODULE.bazel is missing or unreadable, has no `rules_dx`
dependency, or declares or overrides a different version. When MODULE.bazel or an included file cannot be parsed, `dependency` prints `unknown (<reason>)`. Bare `dx version`
still exits `0` on these warnings.

`dx version --check` prints `version ok: <pin>` on stdout. On drift it prints
`dx: status_pin_mismatch: version drift: <pin> != <module>` on stderr and
exits `1`. JSON emits `command_started`, one `pin` status event (`ok` or
`error`), then `command_finished`. On drift a `status_pin_mismatch` error
event comes before `command_finished`.

A missing `.dx/version` makes both forms print no version lines and fail
with `status_pin_mismatch`.

Exit codes: `0` success, `2` usage errors including removed flags, `1` pin
drift under `--check` or a missing `.dx/version`.

```sh
bazel run @rules_dx//:dx -- version
bazel run @rules_dx//:dx -- version --check
```

The [global shim](README.md#global-shim) sets `DX_SHIM=1` for the dx binary
it runs.

## Version Skew

Every command compares the pin in `.dx/version` with the rules_dx module
version built into `dx` before it runs. A mismatch is version skew. For
example, `MODULE.bazel` and `.dx/version` say `1.2.0` and the binary is
`1.3.0`, so `dx build` stops with
`version skew: binary 1.3.0 pin 1.2.0 module 1.3.0`.

- Runs anyway: `version`, `status`, `rollback`, `upgrade`, `completion`, `worktree`.
- Warns and runs: `check`, `security`, `license`, `list-tests`.
- Stops with exit code `1`: every other command.

`watch` answers for the command it
wraps. A missing or empty `.dx/version` is not skew, so other commands run.
An empty pin fails the `pin` check in `dx status` and `dx version --check`.
A missing file makes `dx status` and `dx version` exit `1` with
`status_pin_mismatch`.

A stop prints `dx: version skew: binary <version> pin <pin> module <version>; fix: <hint>`
on stderr. The hint is `dx upgrade --from <pin> --to <module version> --apply`
when the pin is older, otherwise `write <module version> to .dx/version`. Under `--output=json` it adds a `version_skew`
error event, then `command_finished` with `exit_code` `1` and
`results_complete` `false`.

```sh
bazel run @rules_dx//:dx -- version --check
bazel run @rules_dx//:dx -- rollback
bazel run @rules_dx//:dx -- build //...
```
