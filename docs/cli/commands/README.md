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
- [`dx capabilities`](capabilities.md)
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
- `--bazel-startup-option <token>`: Bazel startup option, repeatable. Takes
  `--output_base=<path>` or `--output_user_root=<path>`. Inserted before the
  Bazel verb for every managed invocation. Isolated output bases isolate
  concurrent runs.
- `--report <format>=<dest>`: write a report. Repeatable. Formats per command:
  `sarif` for `security`; `sarif` and `spdx` for `license`; `sarif` for `lint`,
  `typecheck`, `check`, and `fix`; `junit` for `test`; `lcov` for `coverage`.
  Every other command rejects `--report`. A relative `<dest>` anchors at the
  workspace root wherever dx runs; an absolute `<dest>` writes outside the
  workspace as given. Two flags resolving to the same file fail before anything
  runs, including `./` and `parent/../` spellings of one path. Parent directories
  must already exist. A failed write names the resolved file and the cause, exits
  nonzero, and keeps the previous file bytes.
- `--fail-on info|warning|error`: severity that fails. Default `warning`. Taken by
  `security`, `license`, `lint`, `typecheck`, `format`, `check`, and `fix`.
  Every other command rejects it.
- `--min-coverage <percent>`: fail `coverage` below this percent. Taken by
  `coverage`. Every other command rejects it.
- `--strict-evidence`: fail when collected test or coverage evidence is
  incomplete. Taken by `test`, `coverage`. Every other command rejects it.
- `--run-output <dir>`: retain test logs under a unique child directory with
  `manifest.json`. Taken by `test`, `coverage`. Every other command rejects it.
- `--check`: report without changing files. Taken by `lint`, `typecheck`, `format`,
  `generate`, `codegen`, `env`, `setup`, `clean`, `check`, `fix`, `update`,
  `version`, `completion`, and `docs`.
  Every other command rejects it.
- `--apply`: authorize the managed mutation or effect. Taken by `lint`, `typecheck`,
  `format`, `generate`, `run`, `deploy`, `fix`, `clean`, `update`, `bump`, `migrate`,
  `codegen`, `env`, `setup`, `init`, `new`, `upgrade`, `hooks`, `watch`, `version`, `status`, and `docs`.
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
- `capability`: one declared capability, with `name`, `source`,
  `availability`, `detail`, `flags`, `outputs`, `reports`, `scope_policy`,
  `effect`, and `passthrough`.
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
- `test_outcome`: one test result, with `target`, `outcome`, `evidence_complete`,
  and `artifacts`, plus optional `configuration`, `status`, `cached`, `run`,
  `shard`, `attempt`, `duration_millis`, and case counts.
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

`dx.toml` holds committed consumer defaults and `dx.local.toml` holds
optional gitignored local overrides. A key is the flag name without `--`.

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

Keys live under `[dx]`; `dry_run` and `fail_on` spellings also work.
Values are TOML, so a boolean key takes `true` or `false`, not the `<bool>`
spellings above. An empty value is unset. An unknown key is a usage error
that names the key and the keys it accepts. Other top-level tables belong
to other families, for example `[hooks]` in `dx.local.toml`.

The command line wins over the environment, the environment wins over
`dx.local.toml`, `dx.local.toml` wins over `dx.toml`, and `dx.toml` wins
over built-in defaults. Required CI mode ignores `dx.local.toml` and the
`DX_` variables above; explicit flags still apply. `dx status` names the
files in play and the origin of every default: `flag`, `env`, `local`,
`committed`, `legacy`, or `built-in`.

Defaults load from the selected workspace: `--workspace` or `DX_WORKSPACE`
selects it before the file is read. A `workspace` key in the file redirects
once to that workspace. The search reads the working directory and each
directory above it, so a neighboring or nested workspace tree never supplies
defaults. Under `bazel run` the search starts at the workspace root.
`.dx/config.toml` and `.dx/config` are the legacy fallback, read only when
neither `dx.toml` nor `dx.local.toml` exists. When a new file exists next
to legacy state, every command except `dx status --migrate-config` fails
with the conflicting paths; `dx status --migrate-config` plans moving the
legacy keys into `dx.toml`, and `--apply` writes it and removes the legacy
file. `-h`, `--help`, `help`, `-V`, and `--version` print without reading a
broken config. `dx new` and `dx completion` run outside a workspace.
