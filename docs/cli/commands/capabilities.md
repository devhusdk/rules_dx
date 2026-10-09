# `dx capabilities`

```text
dx capabilities [--workspace-capabilities]
```

Prints what this `dx` binary can do, as machines read it. Runs outside a
workspace and downloads nothing. Declared support is not observed
availability: an entry says the grammar accepts the command, not that the
tool is installed.

```text
dx capabilities --workspace-capabilities
```

- Bare run: one record per command, from the built grammar. No workspace
  is needed.
- `--workspace-capabilities`: also print workspace facts from local
  records. Needs a workspace checkout with `MODULE.bazel`. Tool
  availability stays `unknown`: listing never probes tools or
  credentials. Run `dx status` for observed checks.

Output: `--output text|json`.

`--output=json` streams `command_started`, one `capability` event per
command with `name`, `source`, `availability`, `detail`, `flags`,
`outputs`, `reports`, `scope_policy`, `effect`, and `passthrough`, then
`command_finished`. A `capability` source is `cli-grammar`,
`workspace-record`, or `unobserved`. Availability is `available`,
`unavailable`, or `unknown`.

Exit codes: `0` success, `2` usage error, `1` the workspace stage found
no `MODULE.bazel`.

```sh
bazel run @rules_dx//:dx -- capabilities
bazel run @rules_dx//:dx -- capabilities --workspace-capabilities
bazel run @rules_dx//:dx -- capabilities --workspace-capabilities --output=json
```
