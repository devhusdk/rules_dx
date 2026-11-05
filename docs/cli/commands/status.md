# `dx status`

Reports workspace health. Reads only; it downloads, installs and repairs nothing.

```sh
tools/dx status
tools/dx status --output json
tools/dx status --capabilities
tools/dx --version
```

```text
dx status [--capabilities] [--output text|json]
```

| Flag | What it does |
| --- | --- |
| `--capabilities` | Adds one capability record per command and one for `workspace:tools`. |
| `--output text\|json` | Sets the output format. |

Scopes: none.

## Checks

| Check | Reports |
| --- | --- |
| `toolchain` | Rust and `rules_rust` versions declared in `MODULE.bazel`. `warn` when missing, unparsed, overridden or different from the tested versions. |
| `bazel` | `.bazelversion` and `bazel` or `bazelisk` on `PATH`. |
| `module` | `rules_dx` version in `MODULE.bazel` (or an `include()`d file) against the `dx` version. A `single_version_override` version wins over `bazel_dep`. |
| `init_plan` | Read-only `dx init` plan. `warn` names hard conflicts. |
| `host` | Host is linux_x86_64, linux_arm64, macos_arm64, windows_x86_64 or windows_arm64. |
| `exec_tools` | `warn` when `--host_compilation_mode=fastbuild` is not set for `build` or `dx_dev`. |
| `tools` | Tools selected by `dx env --apply`. `warn` when none; `error` when a link or `values.json` is missing or corrupt. |
| `pin` | `.dx/pins` against the `dx` version. |
| `init` | Files `dx init` writes that are missing or out of date. |
| `ide` | After `.dx/ide/state.json` exists: `ok`, or `error` with the failing plug-ins. Fix with `dx env --apply`. |
| `lease` | Inside a `dx worktree` slot: holder and branch. |
| `share` | While build slots are held: held slots and the CPUs and memory a new run gets. |
| `slots` | `warn` for each `slot-N.lock` file dx cannot open. |

`ok` passes. `warn` does not fail. `error` fails; the hint is the fix.

## Output

Text prints one line per check:

```text
toolchain: ok (available: rust 1.99.1 via rules_rust 0.75.0 declared in MODULE.bazel) hint: bazel build //...
```

With `--capabilities`, text adds one `capability <name>` line per command.

JSON streams `command_started`, one `status` event per check (`name`, `status`, `detail`, `hint`), then `command_finished`. With `--capabilities` it adds one `capability` event per command (`name`, `source`, `availability`, `detail`, `flags`, `outputs`, `reports`, `scope_policy`, `effect`, `passthrough`). `source` is `cli-grammar` or `workspace-record`. `availability` is `available`, `unavailable` or `unknown`.

A failed check emits `status_check_failed` naming the checks. A failed `pin` emits `status_pin_mismatch` instead. A missing or empty `.dx/pins` emits `status_pin_mismatch` before any check runs.

```text
dx: status_check_failed: failed checks: module, tools (see hint)
```

`dx --version` prints the binary version and exits `0`.

## Exit Codes

| Code | Meaning |
| --- | --- |
| `0` | All checks pass. |
| `1` | A check failed, or `.dx/pins` is missing. |
| `2` | Usage error. |
