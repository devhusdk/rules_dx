# `dx capabilities`

```sh
bazel run @rules_dx//:dx -- capabilities
bazel run @rules_dx//:dx -- capabilities --workspace-capabilities
```

## `dx capabilities`

```text
dx capabilities [--workspace-capabilities]
```

Prints what this `dx` binary can do, read from its own command table.
Runs outside a workspace and downloads nothing. Takes no scopes.

- Without flags: one block per command with its scopes, outputs, flags,
  reports, and effect authorization.
- With `--workspace-capabilities`: also prints workspace facts from local
  records: the selected workspace, the version pin checks, and an
  availability note. Declared tools and policies are not probed.

Output: `--output text|json`. `--output=json` streams `command_started`,
one `capabilities` event per command, then `command_finished`. With
`--workspace-capabilities` it adds one `status` event per workspace fact
before `command_finished`.

Exit codes: `0` success, `2` usage error, `1` workspace facts unavailable.

A missing or mismatched `.dx/version` fails the workspace stage with
`workspace_unavailable`, but the CLI metadata still prints first. A `pin`
error fails closed. `availability` is advisory and never fails.

```sh
bazel run @rules_dx//:dx -- capabilities --output=json
bazel run @rules_dx//:dx -- capabilities --workspace-capabilities --output=json
```
