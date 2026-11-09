# `dx status`

Reports workspace health. Reads only; it downloads, installs and repairs nothing.

```sh
tools/dx status
tools/dx status --output json
tools/dx status --capabilities
tools/dx --version
```

```text
dx status [--capabilities] [--files] [--output text|json]
```

| Flag | What it does |
| --- | --- |
| `--capabilities` | Adds one capability record per command and one for `workspace:tools`. |
| `--files` | Adds one record per file `dx init` writes: its path, what dx owns (`whole`, `extends`, `keys`, `block`, `lines`, `pin` or `once`) and its state. |
| `--output text\|json` | Sets the output format. |

Scopes: none.

## Checks

| Check | Reports |
| --- | --- |
| `toolchain` | Rust and `rules_rust` versions declared in `MODULE.bazel`. `warn` when missing, unparsed, overridden or different from the tested versions. |
| `toolchain.<lang>` | One record per language found in `MODULE.bazel`, for `python`, `go`, `node`, `jdk`, `dotnet` and `swift`, with the version from its extension tag (`python.toolchain`, `go_sdk.download`, `node.toolchain`, `dotnet.toolchain`, `swift.toolchain`). `warn` with `unknown` when no literal version is declared, as for `jdk`. |
| `bazel` | `.bazelversion` and `bazel` or `bazelisk` on `PATH`. |
| `module` | `rules_dx` version in `MODULE.bazel` (or an `include()`d file) against the `dx` version. A `single_version_override` version wins over `bazel_dep`. |
| `init_plan` | Read-only `dx init` plan. `warn` names hard conflicts. |
| `host` | Host is linux_x86_64, linux_arm64, macos_arm64, windows_x86_64 or windows_arm64. |
| `exec_tools` | `warn` when `--host_compilation_mode=fastbuild` is not set for `build` or `dx_build_dev`. |
| `tools` | Tools selected by `dx sync`. `warn` when none; `error` when a link or `values.json` is missing or corrupt. |
| `pin` | `.dx/pins` against the `dx` version. `warn` when missing, empty, another version, or lacking a sha256 per release asset once the module version is released: launchers run `dx` through Bazel until `dx init --apply` refreshes it. |
| `init` | Files `dx init` writes that are missing or out of date. |
| `ide` | After `.dx/local/ide/state.json` exists: `ok`, or `error` with the failing plug-ins. Fix with `dx sync`. |
| `lease` | Inside a `dx worktree` slot: holder and branch. |
| `share` | While build slots are held: held slots and the CPUs and memory a new run gets. |
| `slots` | `warn` for each `slot-N.lock` file dx cannot open. |
| `dx.toml` | `warn` when `dx.toml` does not parse or its tool configs cannot be bound. |
| `workspace` | `warn` when dx cannot list the workspace root. |

`ok` passes. `warn` does not fail. `error` fails; the hint is the fix.

## Output

Text prints one line per check:

```text
toolchain: ok (available: rust 1.99.1 via rules_rust 0.75.0 declared in MODULE.bazel) hint: bazel build //...
```

After the checks, text prints the tier (unless `dx.toml` fails to parse) and the bound root tool configs:

```text
tier: strict [source: dx.toml]
tier python: standard [source: dx.toml]
config ruff: ruff.toml
```

`tier` is `standard` or `strict`. `source` is `dx.toml` when it sets `tier` and `default` otherwise. A detected, enabled family that sets its own `tier` in `dx.toml` adds a `tier <family>: ...` line. One `config <tool>: <root file>` line follows for each tool of a detected family whose root config file exists; `//.dx:<tool>` binds that file.

With `--capabilities`, text adds one `capability <name>` line per command.

With `--files`, text adds one `file <path> <owns> <state>` line per file `dx init` writes. `state` is `current`, `stale` (a dx-owned value is out of date), `missing` (an essential file is gone), `absent` (an optional file is gone) or `yours` (a `once` file that exists, which dx never rewrites). `docs/cli/commands/init.md` lists the labels.

JSON streams `command_started`, one `status` event per check (`name`, `status`, `detail`, `hint`), then `command_finished`. With `--capabilities` it adds one `capability` event per command (`name`, `source`, `availability`, `detail`, `flags`, `outputs`, `reports`, `scope_policy`, `effect`, `passthrough`). `source` is `cli-grammar` or `workspace-record`. `availability` is `available`, `unavailable` or `unknown`.

JSON adds a `tier` event (`tier`, `source`, and `family` for a per-family tier) and one `tool_config` event (`tool`, `file`) per bound root config, before any `capability` events.

With `--files` JSON adds one `owned_file` event per file (`path`, `owns`, `state`).

A failed check emits `status_check_failed` naming the checks.

```text
dx: status_check_failed: failed checks: module, tools (see hint)
```

`dx --version` prints the binary version and exits `0`.

## Exit Codes

| Code | Meaning |
| --- | --- |
| `0` | All checks pass. |
| `1` | A check failed. |
| `2` | Usage error. |
