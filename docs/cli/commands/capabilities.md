# `dx capabilities`

```text
dx capabilities [--workspace-capabilities] [--output text|json]
dx capabilities --output=json
dx capabilities --workspace-capabilities
dx capabilities --workspace-capabilities --output=json
```

Reports CLI and workspace capabilities as machine-readable metadata. Takes no
scopes. The default stage works outside a workspace and performs no downloads:
it lists every `dx` command with its scope policy, accepted flags, output
modes, reports, effects, and required authorization, all derived from the
command registry and grammar. Pass `--workspace-capabilities` to also report
selected workspace facts read from local records: the workspace root, the
`MODULE.bazel` module name and version, the `.dx/version` pin, and `.dx/config.toml`
presence. Config file contents are never printed.

Each record carries a `state`. `available` means the binary implements it or
the fact was read from disk. `declared` means the registry declares the
support. `unresolved` means the fact needs Bazel analysis or acquisition: tool
availability and the selected quality policy stay `unresolved` because listing
them must not launch probes or downloads. Use `dx status` for observed state.

Output: `--output text|json`.

Exit codes: `0` success, `2` usage error, `1` workspace facts unavailable.

```sh
bazel run @rules_dx//:dx -- capabilities
bazel run @rules_dx//:dx -- capabilities --output=json
bazel run @rules_dx//:dx -- capabilities --workspace-capabilities
```

`--output=json` streams `command_started`, one `capability` event per record
with `kind`, `name`, `state`, `detail`, and structured `data`, then
`command_finished`. Every event carries the CLI `schema` version, so older
readers check the major version and newer fields stay additive.
`--workspace-capabilities` outside a workspace exits `1` with a
`workspace_unresolved` error event.
