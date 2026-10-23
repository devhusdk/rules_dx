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
- [`dx env`, `dx codegen`](environment-codegen.md)
- [`dx security`, `dx license`, `dx update`, `dx bump`](audit-update.md)
- [`dx docs`](docs.md)
- [`dx init`](init.md)
- [`dx status`, `dx version`](status-version.md)
- [`dx list-tests`](list-tests.md)
- [`dx completion`](completion.md)
- [`dx migrate`](migrate.md)
- [`dx new`, `dx upgrade`, `dx rollback`](new-upgrade.md)
- [`dx watch`](watch.md)
- [`dx worktree`](worktree.md)

Scope rules live in [Scope Defaults](scope-defaults.md). There is no `dx doctor`. Use `dx status`.

A `.dx/version` pin that disagrees with the `MODULE.bazel` pin stops most
commands. See [Version Skew](status-version.md#version-skew).

## Inspecting The Graph

`dx` has no owners, deps, or why command. Use native Bazel. Quote every
expression.

```text
bazel query 'rdeps(//..., //pkg:target, 1)'
bazel query 'deps(//pkg:target)'
bazel query 'somepath(//app:server, //lib:core)'
bazel cquery 'deps(//pkg:target)' --platforms=//platforms:linux_x86_64
bazel query 'kind(rule, rdeps(//..., @repo//pkg:target, 1))'
```

`query` reads the unconfigured graph. `cquery` reads the graph for the
selected platform and flags. An empty result is not a pass: check the exit
code.

## Global Flags

Every command takes these:

- `--help` (`-h`): print help.
- `--version` (`-V`): print the dx version.
- `--workspace <dir>`: run in another workspace.
- `--quiet[=bool]` (`-q`), `--verbose[=bool]` (`-v`): less or more output. Bare means
  `true`, and `=false` turns off a default inherited from the environment or a
  config file. Short flags cluster (`-qv`); repeating `-v` changes nothing.
- `--color auto|always|never`: color output. Default `auto`.
- `--bazel-startup-option <token>`: Bazel startup option, repeatable. Takes
  `--output_base=<path>` or `--output_user_root=<path>`. Inserted before the
  Bazel verb for every managed invocation. Isolated output bases isolate
  concurrent runs.

Every other flag belongs to one command family. `dx <command> --help` lists
only the flags that command takes, and a flag from another family is a usage
error.

Read commands take `--output text|json` (`-o`). `-o json` and `-ojson` both
work. Commands that can print a patch also take `diff`. Commands that only
print text take no `--output`.

Check commands take `--fail-on info|warning|error` (default `warning`) and
`--report <format>=<dest>`. Repeat `--report` for more files. A relative
`<dest>` anchors at the workspace root wherever dx runs; an absolute `<dest>`
writes outside the workspace as given. Two flags resolving to the same file fail
before anything runs, including `./` and `parent/../` spellings of one path.
Parent directories must already exist. A failed write names the resolved file and
the cause, exits nonzero, and keeps the previous file bytes. Each command page
lists its report formats. Coverage also takes `--min-coverage <percent>` and
fails below it.

Network commands take `--offline` and `--frozen`. `--offline` means no network:
use pinned cached snapshots or fail. `--frozen` means no manifest or lock
resolution changes and may fetch already pinned content. Combine them for both
policies. Older dx versions spelled offline as `--frozen`: pass `--offline` for
that behavior.

`--check`, `--apply`, `--debug`, `--release`, `--here`, and `--affected` are
documented on the command pages that take them. `--here` limits a command to the
current directory tree. `--affected` limits it to targets your changes reach. `--apply` never combines with `--check`. Bare `--debug` or
`--release` means dev, except for deploy, where it means release.

Exit codes: `0` success, `2` usage error, `1` failed check.

A usage error names the offending token, prints a `tip: a similar ...` line when
a near match exists, then prints the usage line above.

## JSON Output

`--output=json` writes one JSON object per line to stdout. Every object has an
`event` name and a `schema` with `major` and `minor`. The schema is `2.0`.
Readers accept any `2.x` and reject other majors. The stream starts with one
`command_started` and ends with one `command_finished`. Diagnostics and logs go to
stderr. A failure before the command starts emits `error` and then
`command_finished` with no `command_started`; that `command_finished` carries
the process exit code and `results_complete: false`. `DX_OUTPUT=json` and
`output = "json"` in `dx.toml` give the same stream, also when the rest of
`dx.toml` is invalid. A stop signal ends the run with no `error` and no
`command_finished`; dx exits `128` plus the signal number. A closed stdout
exits `141`.

```sh
bazel run @rules_dx//:dx -- lint --check //... --output=json
```

- `command_started`: the command and its mode.
- `build_failure`: one failure Bazel reported while building, from `dx build`,
  `dx test`, and `dx coverage`. It has `record_version`, `target`,
  `configuration`, `target_kind`, `stage`, `category`, `action`, `tool`, and
  `diagnostics`, plus an optional `message`. Facts Bazel did not report read
  `unknown`.
- `capability`: one declared capability, with `name`, `source`,
  `availability`, `detail`, `flags`, `outputs`, `reports`, `scope_policy`,
  `effect`, and `passthrough`.
- `operation`: a phase, with the scopes it resolved to.
- `selection`: the resolved environment, as `setup_id`, `environment_id`, and
  `codegen_id` digests.
- `status`: one check, with `name`, `status`, `detail`, and `hint`.
- `slot`: one worktree slot, with `action`, `path`, `state`, `server`, and
  `disk_bytes`, plus optional `slot`, `branch`, and `holder`.
- `summary`: the pool totals from `dx worktree list`, with `slots`, `servers`,
  `servers_rss_bytes`, and `disk_bytes`.
- `diagnostic`: one finding, with `severity`, `tool`, `message`, `location`,
  `snapshot`, `fixable`, and `count`, plus optional `rule`, `path`, `range`,
  `resolution`, `baseline`, `origins`, and `related`. `location` is `unknown`,
  `file`, `bytes`, or `lines`. A `range` with `lines` location adds
  `start_line`, `start_column`, `end_line`, and `end_column`, all 1-based.
  `count` is how many identical findings this one stands for. `baseline` is
  `suppressed` when the baseline holds the finding. Each of `origins` names the
  `producer`, `target`, and `configuration` that reported it.
- `change`: one file edit, with `path`, `kind`, `edit_count`, and `effect`.
  `effect` is `proposed` in check mode, and `applied` or `refused` with
  `--apply`. A `refused` change carries a `reason`. `--include-edits` adds
  `source_digest` and the full `edits`.
- `phase_started`: a `dx check` or `dx fix` phase began, with `command`,
  `phase`, `mode`, and `correlation`. Every event of the phase carries the same
  `command`, `phase`, and `correlation`.
- `phase_finished`: a phase ended, with its `exit_code` and counts.
- `notice`: a note, with `level`, `code`, `message`, and optional
  `related_command`, `scope`, `path`, `language`, and `import`.
- `report`: a report written, with `format`, `path`, and `results_complete`.
- `test_case`: one case from a test result's report, with `target`, `name`, and
  `outcome`, plus optional `configuration`, `run`, `shard`, `attempt`,
  `cached`, `classname`, `message`, and `message_truncated`.
- `test_outcome`: one test result, with `target`, `outcome`, `evidence_complete`,
  and `artifacts`, plus optional `configuration`, `status`, `cached`, `run`,
  `shard`, `attempt`, `duration_millis`, case counts, and `cases_omitted`.
- `error`: a failure, with `code`, `message`, and optional `cause`, `status`,
  `excerpt`, `truncated`, `path`, `flag`, and `phase`. `cause` is one of
  `input`, `defaults`, `query_launch`, `query`, `analysis`, `bazel`,
  `tool_unavailable`, `collection`, `report`, `operational`, or `unknown`.
  `status` is the exit code of the failed Bazel run. `excerpt` holds the last
  lines Bazel printed to stderr, and `truncated` is `true` when lines were cut.
- `command_finished`: the `exit_code`, plus any `results_complete`,
  `diagnostics`, and `changes` counts. `changes` has `create` and `modify`, and
  with `--apply` also `applied` and `refused`.

`dx run` and `dx update` add `correlation` to their events, so you can group
them by target or set.

## Environment

Every variable below supplies a default. A flag on the command line wins, and
`--quiet=false`, and `--verbose=false` turn an inherited
default off.

- `DX_WORKSPACE=<dir>`: default for `--workspace`.
- `DX_QUIET=<bool>`: default for `--quiet`.
- `DX_VERBOSE=<bool>`: default for `--verbose`.
- `DX_COLOR=<mode>`: default for `--color`.
- `DX_OUTPUT=<mode>`: default for `--output`.
- `DX_FAIL_ON=<level>`: default for `--fail-on`.
- `NO_COLOR=<any>`: disables color in `auto` mode, even when empty. `--color always` still colors.
- `BUILD_WORKSPACE_DIRECTORY=<dir>`: workspace start under `bazel run`.

A `<bool>` is on for `true` and off for `false`, case-insensitive and trimmed.
Any other non-empty value is a usage error. An empty value is unset.

## Config File

`dx.toml` (committed) and `dx.local.toml` (local-only, gitignored) set the
same defaults. A key
is the flag name without `--`.

```toml
[dx]
workspace = "/path/to/repo"
quiet = true
verbose = true
color = "never"
output = "json"
fail-on = "error"
```

Values are TOML, so a boolean key takes `true` or `false`. An empty value is
unset. Defaults live only under `[dx]` with the hyphenated flag names. A
top-level default, `fail_on`, or `dry-run` in either spelling is a usage error.
An unknown key is a usage error that names the key and the keys it accepts.
Other supported sections in the same file stay valid.

Commit shared
defaults in `dx.toml` and keep personal overrides in `dx.local.toml`. Local
beats committed. No other file is read for defaults.
The nearest file to the working directory wins, and its values sit below the environment. The search
reads the working directory and each directory above it, so a neighboring or
nested workspace tree never supplies defaults. Under `bazel run` the search
starts at the workspace root. In CI the local file and `DX_` variables are
ignored. Flags still win. CI never implies `--apply`.

The command line wins over the environment, and the environment wins over
the file. Defaults load from the selected workspace: `--workspace` or
`DX_WORKSPACE` selects it before the file is read. A `workspace` key in the
file redirects once to that workspace. `-h`, `--help`, `help`, `-V`, and
`--version` print without reading a broken config. `dx new` and
`dx completion` run outside a workspace. Deleting `.dx` keeps committed
behavior. Read-only commands never rewrite config.

## Supported Surface

`@rules_dx//:dx` runs every command.
`@rules_dx//rust/rules:defs.bzl` provides Rust wrappers.
`@rules_dx//cc/rules:defs.bzl` provides C and C++ wrappers.
Other files under those directories stay private.
A private file is not a dependency.
Patch releases keep every supported label and flag.
Minor releases add without breaking old uses.
A breaking change needs a new major version and a migration.
Readers tolerate added fields and fail on a new major schema.
Bazel 9.2.0 is the tested baseline.
