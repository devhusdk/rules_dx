# Command Reference

Every `dx` command runs through Bazel. Put `dx` flags after the command,
Bazel flags after `--`. `@rules_dx//:dx` is the public launcher; inside this
checkout `//:dx` is the same target.

```sh
bazel run @rules_dx//:dx -- build //...
bazel run @rules_dx//:dx -- lint --check //... -- --jobs=4
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
- `--dry-run[=bool]`: print the plan without running it. Bare means `true`, and
  `=false` turns off a default inherited from the environment or a config file.
- `--quiet[=bool]`, `--verbose[=bool]`: less or more output. Bare means `true`,
  and `=false` turns off a default inherited from the environment or a config
  file.
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
- `--min-coverage <percent>`: fail `coverage` below this percent. Taken by
  `coverage`. Every other command rejects it.
- `--check`: report without changing files. Taken by `lint`, `typecheck`, `format`,
  `generate`, `codegen`, `env`, `setup`, `clean`, `check`, `fix`, `update`,
  `version`, `completion`, and `docs`.
  Every other command rejects it.
- `--apply`: authorize the managed mutation or effect. Taken by `lint`, `typecheck`,
  `format`, `generate`, `run`, `deploy`, `fix`, `clean`, `update`, `bump`, `migrate`,
  `codegen`, `env`, `setup`, `init`, `new`, `upgrade`, `hooks`, `watch`, `version`, and `docs`.
  Every other command rejects it. It never combines with `--check` or `--dry-run`.
- `--debug`, `--release`: build profile, mutually exclusive. Taken by `build`,
  `test`, `run`, and `deploy`. Bare means dev, except `dx deploy` which means
  release. Every other command rejects them.
- `--offline`, `--frozen`: run cache-only, no network fetches. Taken by
  `security`, `license`, `update`, and `bump`. Every other command rejects it.
- `--here`: limit to the current directory tree.
- `-h`, `--help`: print help for a command.
- `-V`, `--version`: print the version.

Exit codes: `0` success, `2` usage error, `1` failed check.

A usage error names the offending token, prints a `tip: a similar ...` line when
a near match exists, then prints the usage line above.

## JSON Output

`--output=json` writes one JSON object per line to stdout. Every object has an
`event` name and a `schema` with `major` and `minor`. The stream starts with
`command_started` and ends with `command_finished`. Diagnostics and logs go to
stderr.

```sh
bazel run @rules_dx//:dx -- lint --check //... --output=json
```

- `command_started`: the command, whether it is a dry run, and its mode.
- `operation`: a phase, with the scopes it resolved to.
- `selection`: the resolved environment, as `setup_id`, `environment_id`, and
  `codegen_id` digests.
- `status`: one check, with `name`, `status`, `detail`, and `hint`.
- `diagnostic`: one finding, with `severity`, `tool`, `message`, and optional
  `rule`, `path`, `range`, `snapshot`, `fixable`, and `resolution`.
- `change`: a file the command would edit, with `path`, `kind`, and `edits`.
- `mutation`: a file the command edited or skipped, with `path`, `kind`,
  `outcome`, and an optional `reason`.
- `notice`: a note, with `level`, `code`, `message`, and optional
  `related_command`, `scope`, `path`, `language`, and `import`.
- `report`: a report written, with `format`, `path`, and `results_complete`.
- `error`: a failure, with `code`, `message`, and optional `path`, `flag`, and
  `phase`.
- `command_finished`: the `exit_code`, plus any `results_complete`,
  `diagnostics`, `changes`, and `mutations` counts.

`dx run` and `dx update` add `correlation` to their events, so you can group
them by target or set.

## Environment

Every variable below supplies a default. A flag on the command line wins, and
`--dry-run=false`, `--quiet=false`, and `--verbose=false` turn an inherited
default off.

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

A `<bool>` is on for `1`, `true`, `yes`, `y`, or `on`, and off for `0`, `false`,
`no`, `n`, or `off`. Any other non-empty value is a usage error. An empty value
is unset.

## Config File

`dx.toml` and `dx.local.toml` set the same defaults. A key is the flag
name without `--`. Commit `dx.toml`; keep local-only values in
`dx.local.toml`, which is gitignored. The local file wins per key.

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
An unknown key is a usage error that names the key and the keys it accepts.

Keys go under `[dx]` or at the top level, and `[dx]` wins. The nearest
`dx.toml` to the working directory wins, and separately the nearest
`dx.local.toml` wins. The search reads the working directory and each
directory above it, so a neighboring or nested workspace tree never supplies
defaults. Under `bazel run` the search starts at the workspace root.

`.dx/config.toml` and `.dx/config` are the legacy fallback. They load only
when neither `dx.toml` nor `dx.local.toml` exists. The nearest file to the
working directory wins, and its values sit below the environment. When a
directory holds both `.dx/config.toml` and `.dx/config`, the `.toml` one
wins. A legacy file next to a new file is a usage error that names both
files: move its keys into `dx.toml` (`dx.local.toml` for local-only values)
and delete it. `dx` never migrates config on its own, and no default command
writes a config file. Deleting `.dx` keeps committed and local defaults
working.

The command line wins over the environment, the environment wins over the
local file, the local file wins over the committed file, and the committed
file wins over built-in defaults. `dx status` reports the effective origin
in its `config` check. Under CI (`CI=true`) the local file, the legacy
fallback, and the preference environment variables are ignored, so required
runs see committed defaults plus explicit flags. `DX_WORKSPACE` still
selects the workspace under CI. Defaults load from the selected workspace:
`--workspace` or `DX_WORKSPACE` selects it before the file is read. A
`workspace` key in the file redirects once to that workspace. `-h`,
`--help`, `help`, `-V`, and `--version` print without reading a broken
config. `dx new` and `dx completion` run outside a workspace.
