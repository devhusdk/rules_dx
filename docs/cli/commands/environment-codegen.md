# Environment And Codegen Commands

```sh
bazel run @rules_dx//:dx -- env
bazel run @rules_dx//:dx -- codegen
```

```text
dx env [--check] [--apply] [<label>] [-- bazel-options...]
dx codegen [--check] [--apply] [<label>] [-- bazel-options...]
```

`dx env` collects the managed tools and env values. It upgrades nothing.
`dx codegen` materializes declared generated source. Both are
repository-wide by default.

With one exact `//` or `@` label, `dx env` acts on that target only.
`dx codegen <label>` builds that label plus every codegen shard that
depends on it. For example, `dx codegen //proto:api` also builds each
codegen shard that depends on `//proto:api`. dx finds the shards with
`bazel query "kind('.*codegen_shard rule', rdeps(//..., set(<label>)))"`.

A pattern, a relative label, a path, an empty value or a second label is
a usage error and exits `2`.

Both check the intended selection by default and report drift without
writing. Pass `--apply` to select it. A missing selection is drift, not
success. Checking never selects a new generation, populates managed
bins, or commits records.

Both commands also pass
`--@rules_dx//config:validate=false`, `--@rules_dx//config:clippy_output_diagnostics=true`
and `--@rules_dx//config:rustc_output_diagnostics=true`. Another value for one of them,
or for the matching `@rules_rust//rust/settings` flag, fails before Bazel starts with exit `2`.
They reject `--remote_download_minimal`, `--remote_download_outputs=minimal`,
`--build_event_json_file` and any `--build_event_json_file_path_conversion`
other than `false` the same way.

These commands keep managed state under `.dx/`, which is gitignored.
`dx clean` reports unselected managed state. `dx clean --apply` prunes it.
They take no `--fail-on` or `--report`.

To run both, run `env` first, then `codegen`. Stop on the first failure.

```sh
bazel run @rules_dx//:dx -- env --apply
bazel run @rules_dx//:dx -- codegen --apply
```

`dx setup` is not a command. It exits `2` and names `dx env` and
`dx codegen`.

Output: `--output text|json`.

Text output starts with `Running <command> for <roots>`. Then dx prints
one result line, where `<pair>` is `(environment <id>, generated <id>)`:

- Up to date: `dx <command>: already selected setup <setup> <pair>`.
- Drift with a selection:
  `dx <command>: stale selection: current setup <setup> want setup <setup> <pair>`.
- Drift with no selection:
  `dx <command>: no current selection: intended setup <setup> <pair>`.
- With `--apply`: `dx <command>: selected setup <setup> <pair>`, or
  `already selected setup` when nothing changed.

Drift adds `` run `dx <command> --apply [<label>]` to select it ``.
`--quiet` hides all of these lines, drift included. The exit status stays
the same.

JSON output writes these events in order:

- `command_started`, with mode `check` or `default` (`--apply`).
- `operation`, with phase `collect`. Its `scope` is the label, or the
  codegen roots after expansion. The whole repository has no `scope`.
- On drift, a `notice` with code `managed_drift`.
- With `--apply`, a `selection` with `setup`, `environment` and
  `generated`.
- When Bazel fails, an `error` with code `bazel_failed` and phase
  `collect`.
- Last, `command_finished`.

Other failures write an `error` with its code, then `command_finished`.
Codes include `invalid_arguments`, `invalid_result`, `no_capability`,
`unreadable_bep` and `managed_commit_failed`.

Exit codes: `0` success. `2` usage or scope errors: a bad scope, a
conflicting validate or diagnostics flag, `--aspects`, `--output_groups`
or `--build_event_json_file` after `--`, `dx setup`, or a failed codegen
label query. The JSON `error` keeps the query status. `1` drift, an
invalid collection result (`invalid_result`), no capability
(`no_capability`), a non-UTF-8 temporary event path (`unreadable_bep`),
a commit failure (`managed_commit_failed`), or a Bazel launch failure.
Bazel failures keep Bazel's code.

```sh
bazel run @rules_dx//:dx -- env //a:one
bazel run @rules_dx//:dx -- codegen //proto:api
bazel run @rules_dx//:dx -- codegen --apply
bazel run @rules_dx//:dx -- env --apply
```
