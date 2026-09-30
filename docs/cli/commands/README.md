# Command Reference

Every `dx` command runs through Bazel. Put `dx` flags before the command,
Bazel flags after `--`.

```sh
bazel run //cli/cli:dx -- build //...
bazel run //cli/cli:dx -- lint --check //... -- --jobs=4
```

- [`dx build`, `dx test`, `dx coverage`, `dx run`, `dx deploy`](build-test-coverage.md)
- [`dx lint`, `dx typecheck`, `dx format`](quality.md)
- [`dx check`, `dx fix`, `dx clean`](check-fix-clean.md)
- [`dx generate`](generate.md)
- [`dx env`, `dx codegen`, `dx setup`](environment-codegen-setup.md)
- [`dx security`, `dx license`, `dx update`, `dx bump`, `dx bazel`](audit-update-bazel.md)
- [`dx docs`](docs.md)
- [`dx init`, `dx hooks`](hooks.md)
- [`dx status`, `dx version`](status-version.md)
- [`dx owners`, `dx deps`, `dx why`](inspect.md)
- [`dx completion`](completion.md)
- [`dx migrate`](migrate.md)
- [`dx new`, `dx upgrade`](new-upgrade.md)
- [`dx watch`](watch.md)

Scope rules live in [Scope Defaults](scope-defaults.md). There is no `dx doctor`. Use `dx status`.

A `.dx/version` pin that disagrees with the `MODULE.bazel` pin stops most
commands. See [Version Skew](status-version.md#version-skew).

## Global Flags

- `--workspace <dir>`: run in another workspace.
- `--dry-run`: print the plan without running it.
- `--quiet`, `--verbose`: less or more output.
- `--log-level error|warn|info|debug|trace`: log verbosity.
- `--color auto|always|never`: color output. Default `auto`.
- `--output text|diff|json`: result shape. `diff` is limited to `lint`,
  `typecheck`, `format`, `generate`, `check`, and `fix`. `json` is rejected by
  `deploy`, `init`, `new`, `hooks`, `watch`, `completion`, and `bazel`, which
  take `text` only.
- `--report <format>=<dest>`: write a report. Repeatable. Formats per command:
  `sarif` for `security`; `sarif` and `spdx` for `license`; `sarif` for `lint`,
  `typecheck`, `check`, and `fix`; `junit` for `test`; `lcov` for `coverage`.
  Every other command rejects `--report`.
- `--fail-on info|warning|error`: severity that fails. Default `warning`. Taken by
  `security`, `license`, `lint`, `typecheck`, `format`, `check`, and `fix`.
  Every other command rejects it.
- `--min-coverage <percent>`: fail `coverage` below this percent.
- `--check`: report without changing files. Taken by `lint`, `typecheck`, `format`,
  `generate`, `check`, `fix`, `update`, `version`, `completion`, and `docs`.
  Every other command rejects it.
- `--debug`, `--release`: build profile, mutually exclusive. Taken by `build`,
  `test`, `run`, and `deploy`. Bare means dev, except `dx deploy` which means
  release. Every other command rejects them.
- `--offline`, `--frozen`: run cache-only, no network fetches. Taken by
  `security`, `license`, `update`, and `bump`. Every other command rejects it.
- `--here`: limit to the current directory tree.
- `-h`, `--help`: print help for a command.
- `-V`, `--version`: print the version.

Exit codes: `0` success, `2` usage error, `1` failed check.

## Environment

Every variable below supplies a default. A flag on the command line wins.

- `DX_WORKSPACE=<dir>`: default for `--workspace`.
- `DX_DRY_RUN=<bool>`: default for `--dry-run`.
- `DX_QUIET=<bool>`: default for `--quiet`.
- `DX_VERBOSE=<bool>`: default for `--verbose`.
- `DX_COLOR=<mode>`: default for `--color`.
- `DX_OUTPUT=<mode>`: default for `--output`.
- `DX_FAIL_ON=<level>`: default for `--fail-on`.
- `RUST_LOG=<filter>`: overrides `--verbose` and `--log-level`.
- `NO_COLOR=<any>`: disables color output.
- `BUILD_WORKSPACE_DIRECTORY=<dir>`: workspace start under `bazel run`.

A `<bool>` is on for `1`, `true`, `yes`, `y`, or `on`.

## Config File

`.dx/config.toml` and `.dx/config` set the same defaults. A key is the flag
name without `--`.

```toml
[dx]
workspace = "/path/to/repo"
dry-run = true
quiet = true
verbose = true
color = "never"
output = "json"
fail-on = "error"
```

`dry_run` and `fail_on` also work. Values are TOML, so a boolean key takes
`true` or `false`, not the `<bool>` spellings above. An empty value is unset.
An unknown key is ignored.

Keys go under `[dx]` or at the top level, and `[dx]` wins. When a directory
holds both `.dx/config.toml` and `.dx/config`, the `.toml` one wins. The
nearest file to the working directory wins, and its values sit below the
environment. Under `bazel run` the search starts at the workspace root.
