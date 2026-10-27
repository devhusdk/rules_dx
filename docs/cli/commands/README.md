# Command Reference

Every `dx` command runs through Bazel. Put `dx` flags after the command,
Bazel flags after `--`. `@rules_dx//:dx` is the public launcher; inside this
checkout `//:dx` is the same target.

Locally, Bazel reads `~/.bazelrc` and the system rc for every `dx` call. Under
`CI=true` and inside Bazel tests, `dx` passes `--nohome_rc --nosystem_rc`.

```sh
bazel run @rules_dx//:dx -- build //...
bazel run @rules_dx//:dx -- lint --check //... -- --jobs=4
```

- [`dx build`, `dx test`, `dx coverage`, `dx run`, `dx deploy`](build-test-coverage.md)
- [`dx lint`, `dx typecheck`, `dx format`](quality.md)
- [`dx check`, `dx fix`, `dx clean`](check-fix-clean.md)
- [`dx generate`](generate.md)
- [`dx env`, `dx codegen`](environment-codegen.md)
- [`dx security`, `dx license`, `dx update`, `dx bump`, `dx add`](audit-update.md)
- [`dx docs`](docs.md)
- [`dx init`](init.md)
- [`dx status`, `dx version`](status-version.md)
- [`dx list-tests`](list-tests.md)
- [`dx completion`](completion.md)
- [`dx migrate`](migrate.md)
- [`dx new`, `dx upgrade`, `dx rollback`](new-upgrade.md)
- [`dx watch`](watch.md)
- [`dx worktree`](worktree.md)

Scope rules live in [Scope Defaults](scope-defaults.md).

Airgapped hosts install from a vendored bundle: see
[Offline Bundle](offline-bundle.md).

A `.dx/version` pin that disagrees with the rules_dx version `dx` was built
from stops most commands. See [Version Skew](status-version.md#version-skew).

## Build Slots

`dx build`, `test`, `coverage`, `deploy`, `lint`, `typecheck`,
`format`, `check`, `fix`, `generate`, `codegen`, `env`, `security`, `license`,
`docs` and `list-tests` take a build slot before they call Bazel.
`dx run` holds a slot only while it builds and frees it before the app starts.
`dx watch` takes one slot per iteration and holds none while it waits for
changes. `dx update` takes one slot at its first updater that runs Bazel and
holds it until its updaters finish. `dx update --check` and updaters that do
not run Bazel take none. Slots are shared by every
worktree and every user session on the machine. The slot count is the machine
memory, or the cgroup memory limit if lower, divided by 3 GiB. There is always
at least one and never more than 16.

`--build-slots auto|<n>|off` sets the count for one run. `DX_BUILD_SLOTS` and
`build-slots` in `dx.toml` or `dx.local.toml` set a default, and the flag wins.
The default is `auto`, the count above. In CI the env var and
`dx.local.toml` are ignored, like every other default.

- `auto`: the memory-based count above, with the remote extras below.
- `<n>`: exactly `n` slots, from 1 to 1024. There is no 16 cap and no remote
  extras; raise `n` instead. In the rules_dx workspace the slots kept for
  nested Bazel test servers sit above `n`.
- `off`: dx takes no slot, never waits for one, and adds no
  `--local_resources` flags.

Every dx process applies its own setting, while the slot files are shared
across the machine. A run with `--build-slots 32` can take slots that `auto`
runs never look at, so the setting is not a machine-wide cap. `DX_BUILD_SLOT`
(singular) is an internal marker dx sets for nested calls; do not set it.

When the Bazel config sets a non-empty `--remote_executor`, dx offers more
slots: the memory divided by 1.5 GiB, at most 16. dx reads this from the rc
files Bazel reads, their `--config` sections, and the flags you pass. A run
takes an extra slot only when no regular slot is free, the host is not under
memory pressure, and no other run took an extra slot in the last 10 seconds.
On Linux, memory pressure means `/proc/pressure/memory` `some avg10` is 10 or
more, or `MemAvailable` is below 1.5 GiB when that file is missing. macOS and
Windows get no extra slots. A run that waits only because of memory pressure
says so in its waiting notice.

While a slot is held, dx adds two flags right after `build`, `test`,
`coverage` and `run`:

```text
--local_resources=cpu=<n> --local_resources=memory=<mb>
```

`n` is the CPU count divided by the slots held right now, extra slots and
this one included, and never less than 2. `mb` is the memory, less the memory kept for nested
Bazel test servers, divided by the same number, in MiB. The values are fixed
when dx takes the slot. Nested dx calls reuse them through `DX_BUILD_SHARE`.

dx adds neither flag when an rc file Bazel reads sets that resource to a value
other than the vendored preset default. It also skips a resource you pass after
`--`:

```bash
dx build //... -- --local_resources=cpu=4
```

CI runs and dx runs inside a Bazel test add neither flag.

When all slots are taken, or other runs already wait, dx waits and prints one
line to stderr:

```text
dx: waiting for a build slot (held by /path/to/worktree and 5 more, 2 ahead in line)
```

The line names one current holder, how many other slots are held, and how many
runs wait ahead. Waiting runs get a slot in the order they started waiting. A
waiter that exits or is stopped leaves the line. A waiter suspended for over 30
seconds does not hold up the line until it resumes. If dx cannot use the
`queue` directory next to the slot files, it waits without a line.

With `--output=json`, dx also writes one `notice` event with code
`waiting_for_build_slot` right after `command_started`. The slot frees when the
Bazel work ends or dx exits, including when it crashes.

While dx waits, it keeps the worktree's running Bazel server from shutting down
for idleness. It never starts a server.

If dx cannot write a slot file, it locks it read-only. If it still cannot use
a slot file, it prints `dx: skipped build slot <path>: <error>` once and runs
with one slot fewer. With `--output=json` it also writes one warning `notice`
event with code `build_slot_skipped`.

If dx can use no slot file, it prints `dx: build slots disabled: <error>` and
runs without a slot. With
`--output=json` it also writes one `notice` event with code
`build_slots_disabled`.

Ctrl-C or SIGTERM during the wait stops dx at once. dx prints
`dx: cancelled: stopped by signal N` and exits `128` plus the signal number.

`status`, `version`, `worktree`, `completion` and the other commands that do not
run Bazel never take a slot. A dx call started while `DX_BUILD_SLOT` is set, or
from inside a Bazel test, takes no second slot. While dx holds a slot, the
Bazel and tool processes it starts see `DX_BUILD_SLOT=1` and `DX_BUILD_SHARE`
set to `cpu=<n>,memory=<mb>`. `dx run` apps start
after the slot frees.

## Disk Space

Before each Bazel build it starts, dx checks free space on the workspace disk,
the Bazel output base, the temp dir and the home dir. When one has under 1 GiB
free, dx prints one line to stderr and keeps going:

```text
dx: disk_low: 512.0 MiB free at /path; free disk space with `dx worktree prune --apply`
```

With `--output=json`, dx also writes one `notice` event with code `disk_low`
and level `warning`.

`dx run` and `dx deploy` check before the build, not before the app starts, and
an app's exit code is never reported as `disk_full`.

When the disk holding the worktree pool has under 20 GiB and under 10% free,
or under 1 GiB free,
dx drops orphaned output bases, repository cache downloads unused for 30 days,
the output bases of free nested-Bazel test slots, and free slots' output bases each time it
takes a build slot: once for each command above, for each `watch` iteration,
for the build `run` does before the app starts, and for `update` when it runs
Bazel. This also happens when build slots are disabled. It never drops the
output base of the workspace being built. dx prints one of these lines to
stderr only when it removed something or could not reclaim:

```text
dx: low disk: <free> free; removed <what>; <free> free now
dx: low disk, cannot reclaim output bases: <error>
```

With `--output=json`, dx also writes one `notice` event with code `disk_low`
and level `warning`, after `command_started` and before that build's output.

`dx worktree add` and `dx worktree hook create` also refuse with code
`disk_full` when the pool's disk has under 1 GiB free after that reclaim.

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
- `--workspace <dir>`: run in another workspace. A relative dir starts from the current directory.
- `--quiet[=bool]` (`-q`), `--verbose[=bool]` (`-v`): less or more output. Bare means
  `true`, and `=false` turns off a default inherited from the environment or a
  config file. Short flags cluster (`-qv`); repeating `-v` changes nothing.
- `--color auto|always|never`: color output. Default `auto`.
- `--build-slots auto|<n>|off`: build slots for this run. Default `auto`. See
  [Build Slots](#build-slots).
- `--bazel-startup-option <token>`: Bazel startup option, repeatable. Takes
  `--output_base=<path>` or `--output_user_root=<path>`. Inserted before the
  Bazel verb for every managed invocation. Isolated output bases isolate
  concurrent runs.

Every other flag belongs to one command family. `dx <command> --help` lists
only the flags that command takes, and a flag from another family is a usage
error.

Read commands take `--output text|json` (`-o`). `-o json`, `-ojson` and short
clusters such as `-qojson` all work. Commands that can print a patch also take `diff`. Commands that only
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
a near match exists, then prints the usage line above. With no command, the usage
error prints the command list and a `see dx <command> --help` hint.

## JSON Output

`--output=json` writes one JSON object per line to stdout. Every object has an
`event` name and a `schema` with `major` and `minor`. The schema is `3.3`.
Readers accept any `3.x` and reject other majors. The stream starts with one
`command_started` and ends with one `command_finished`. Diagnostics and logs go to
stderr. A failure before the command starts emits `error` and then
`command_finished` with no `command_started`; that `command_finished` carries
the process exit code and `results_complete: false`. If dx cannot build an
event, it writes `error` with code `invalid_result`, then `command_finished`,
and exits `1`. `DX_OUTPUT=json` and
`output = "json"` in `dx.toml` give the same stream, also when the rest of
`dx.toml` is invalid. A stop signal ends the run with no `error` and no
`command_finished`; dx exits `128` plus the signal number. A closed stdout
exits `141`.

```sh
bazel run @rules_dx//:dx -- lint --check //... --output=json
```

### Agent sessions

When `CLAUDECODE`, `CODEX_THREAD_ID`, `CURSOR_AGENT`, `GEMINI_CLI`, or
`AI_AGENT` is set, the default output is `json` for commands that support it,
and Bazel progress stays off stderr. `--output` and `DX_OUTPUT` still win, and
so does `output` in `dx.toml`. In CI these variables are ignored.

Empty optional fields are left out of `test_outcome` and `build_failure` events. A plain
pass is not an event. `dx test` and `dx coverage` count passed, failed, cached, no_cases, and skipped tests
in `command_finished` and write every test to the `results` report. Each count is
per result, so a target with four shards counts four. Equal
`build_failure` and `diagnostic` events from different targets and
configurations appear once, with a `count`.

- `command_started`: the command and its mode.
- `build_failure`: one failure Bazel reported while building, from `dx build`,
  `dx test`, and `dx coverage`. It has `record_version`, `target`,
  `configuration`, `target_kind`, `stage`, `category`, `action`, `tool`,
  `message`, and `diagnostics`, whose items may carry a `related` list. Facts Bazel did not report read `unknown`. The
  same failure from several targets or configurations is one event with a
  `count` and up to 20 other `targets`.
- `capability`: one declared capability, with `name`, `source`,
  `availability`, `detail`, `flags`, `outputs`, `reports`, `scope_policy`,
  `effect`, and `passthrough`.
- `operation`: a phase, with the scopes it resolved to.
- `selection`: the resolved environment, as `setup_id`, `environment_id`, and
  `codegen_id` digests.
- `status`: one check, with `name`, `status`, `detail`, and `hint`.
- `slot`: one worktree slot, with `action`, `path`, `state`, and `server`,
  plus optional `slot`, `branch`, `holder`, `reason`, `base`, and
  `disk_bytes`. `reason` says why a slot or worktree was kept, or that a held slot's
  holder is gone.
- `coverage`: one cell's owned-source line account from `dx coverage`, with
  `cell`, `covered_lines`, `eligible_lines`, `raw_lines`, `excluded_lines`,
  `eligible_files`, `support_files`, `excluded_files`, `unsupported_files`,
  `passed`, `errors`, and `dx_version`, plus optional `minimum`.
- `changed_line_coverage`: the `--min-changed-line-coverage` gate from
  `dx coverage`, with `base`, `merge_base`, `minimum`, `covered_lines`,
  `eligible_lines`, `uncovered` (`path` and `line` items), `uninstrumented`,
  `result` (`pass`, `fail`, `not-applicable`, `incomplete`, or
  `unavailable`), and `dx_version`.
- `summary`: the pool totals from `dx worktree list`, with `slots`, `servers`,
  and `servers_rss_bytes`, plus optional `disk_bytes` and `disk_cache_bytes`.
- `diagnostic`: one finding, with `severity`, `tool`, `message`, `location`,
  `snapshot`, `fixable`, and `count`, plus optional `rule`, `path`, `range`,
  `package`, `resolution`, `baseline`, `origins`, and `related`. `location` is
  `unknown`, `file`, `bytes`, or `lines`. A `range` with `lines` location adds
  `start_line`, `start_column`, `end_line`, and `end_column`, all 1-based.
  `count` is how many identical findings this one stands for. `baseline` is
  `suppressed` when the baseline holds the finding. Each of `origins` names the
  `producer`, `target`, and `configuration` that reported it. `package` names
  the dependency of a `vuln` or `license` finding, with `name` and `version`,
  plus `fixed` for a vulnerability and `license` for a license finding. An
  empty `fixed` list means no fixed version exists.
- `change`: one file edit, with `path`, `kind`, `edit_count`, and `effect`.
  `effect` is `proposed` in check mode, and `applied` or `refused` with
  `--apply`. A `refused` change carries a `reason` and makes the run exit `1`. `--include-edits` adds
  `source_digest` and the full `edits`.
- `phase_started`: a `dx check` or `dx fix` phase began, with `command`,
  `phase`, `mode`, and `correlation`. Every event of the phase carries the same
  `command`, `phase`, and `correlation`.
- `phase_finished`: a phase ended, with its `exit_code` and counts.
- `notice`: a note, with `level`, `code`, `message`, and optional
  `related_command`, `scope`, `path`, `language`, and `import`.
- `report`: a report written, with `format`, `path`, and `results_complete`.
  `format` is `junit`, `lcov`, `failures`, or `results`.
- `test_outcome`: one test result except a plain pass, with `target` and
  `outcome`, plus optional `configuration`, `status`, `cached`, `run`, `shard`,
  `attempt`, `final`, `duration_millis`, `cases_total`, `cases_failed`,
  `cases_error`, `cases_skipped`, `cases_omitted`, `artifacts`, and
  `evidence_complete`. `evidence_complete` and `artifacts` appear only when
  evidence is missing. `final` is `false` only on an attempt that a later
  attempt replaced.
  A failing result adds `failing_cases` (up to 10, each with `classname`,
  `name`, `outcome`, and a `message` cut to 1024 bytes),
  `failing_cases_omitted`, `log_path`, and `log_tail` (the last 40 lines, at most 8 KiB of the
  test log).
- `error`: a failure, with `code`, `message`, and optional `cause`, `status`,
  `excerpt`, `truncated`, `path`, `flag`, and `phase`. `cause` is one of
  `input`, `defaults`, `query_launch`, `query`, `analysis`, `bazel`, `test`,
  `application`, `tool_unavailable`, `collection`, `report`, `host`,
  `operational`, or `unknown`. `bazel_failed` has cause `bazel`; `tests_failed`
  has cause `test`; `application_failed` is a
  `dx run` application that exited nonzero or died by a signal, and has cause
  `application`. For a signal, `status` is `128` plus the signal number.
  `disk_full` has cause `host`: a Bazel run or query exited 36 and a disk it
  writes to has under 1 GiB free. dx keeps exit 36 and prints a `next:` line suggesting
  `dx worktree prune --apply`.
  `status` is the exit code of the failed Bazel run or application. `excerpt` holds the last
  lines Bazel printed to stderr, and `truncated` is `true` when lines were cut.
- `command_finished`: the `exit_code`, plus any `results_complete`, `run_id`,
  `diagnostics`, `tests`, and `changes` counts. `tests` has `passed`,
  `failed`, `cached`, `no_cases`, and `skipped`. Each counts results (per target,
  configuration, run and shard), so a target with four shards counts four. `failed` counts results whose final attempt has
  outcome `failed` or `timeout`; earlier failed attempts of a retried test
  still appear as `test_outcome` events with `final: false` and are not
  counted. `cached` counts
  the passed tests that came from the cache. `no_cases` counts passed tests
  whose report had no test cases; they are also in `passed`. `skipped` counts tests
  with outcome `skipped` or `filtered`. `changes` has `create` and
  `modify`, and with `--apply` also `applied` and `refused`. `actions` has
  the Bazel action counts summed over the command's builds: `created`,
  `executed`, and `cache_hits` with `disk`, `remote`, and `local`.
  `executed` leaves out disk and remote cache hits, and is approximate when an
  action runs several spawns. `actions` is absent when no build reported
  metrics.

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
- `DX_BUILD_SLOTS=auto|<n>|off`: default for `--build-slots`.
- `NO_COLOR=<any>`: disables color in `auto` mode, even when empty. `--color always` still colors.
- `BUILD_WORKSPACE_DIRECTORY=<dir>`: workspace start under `bazel run`.
- `BUILD_WORKING_DIRECTORY=<dir>`: current directory under `bazel run`.

A `<bool>` is on for `true` and off for `false`, case-insensitive and trimmed.
Any other non-empty value is a usage error. An empty value is unset.
A `DX_OUTPUT` or `DX_FAIL_ON` value that a command does not support is ignored
for that command, as in the config file.

## Global Shim

The shim is a small `dx` binary for PATH, built from `//cli/shim`. It runs the
dx version a workspace pins without Bazel and never edits the workspace.

1. The version is `DX_VERSION` when set, otherwise the first line of the
   nearest `.dx/version` above the current directory (CR and spaces stripped).
   Both must be plain semver.
2. The binary is `<cache>/bin/<version>/dx-<os>-<arch>` (`.exe` on Windows),
   the same file `tools/dx` uses. A cached binary runs with no network access.
3. A missing binary is downloaded from the GitHub release, checked against the
   release's `.sha256` file, and moved into place. A binary that does not match
   is deleted and never run.
4. The shim runs the binary with the same arguments, stdin, stdout and stderr,
   and sets `DX_SHIM=1`.

`dx shim version` prints `dx-shim <version>`. `dx shim where` prints the
version, where it came from, the binary path, and whether it is cached. It
never downloads. The dx binary rejects `shim` as a command name.

Environment:

- `DX_VERSION=<semver>`: run this version instead of the `.dx/version` pin.
- `DX_CACHE_DIR=<dir>`: cache root. Otherwise `$XDG_CACHE_HOME/dx`, then
  `~/.cache/dx` (`%USERPROFILE%\.cache\dx` on Windows). Empty values are unset.
- `DX_RELEASE_BASE_URL=<url>`: download from `<url>/v<version>/<asset>`
  instead of the GitHub release. Must be `https://`, or `http://` to
  `127.0.0.1`, `[::1]` or `localhost`. The shim trusts this server and its
  `.sha256` files fully.

Errors print one `dx: ` line on stderr. Exit codes: `0` success, `2` usage
errors (bad `DX_VERSION`, bad `DX_RELEASE_BASE_URL`, unknown `dx shim`
command), `1` other failures (no `.dx/version`, no binary for the platform,
download failure, sha256 mismatch, cache not writable). Otherwise the exit code
or signal is the dx binary's own.

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
build-slots = "auto"
```

Values are TOML, so a boolean key takes `true` or `false`. An empty value is
unset. Defaults live only under `[dx]` with the hyphenated flag names. A
top-level default or `fail_on` is a usage error.
An unknown key is a usage error that names the key and the keys it accepts.
Other supported sections in the same file stay valid.

Unsupported defaults are ignored per command. An `output` default that a
command does not support falls back to the command's default output: `json` in
an agent session for commands with JSON output, else `text`. For example, with
`output = "diff"` an agent session runs `dx test` with `json`. An unsupported
`fail-on` default falls back to `warning`. The same value as a flag is a usage
error. An invalid value is always a usage error.

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
file redirects once to that workspace. A relative `workspace` key starts
from the workspace start. `-h`, `--help`, `help`, `-V`, and
`--version` print without reading a broken config. `dx new` and
`dx completion` run outside a workspace. Deleting `.dx` keeps committed
behavior. Read-only commands never rewrite config.

## Supported Surface

`@rules_dx//:dx` runs every command.

Language rules:

- `@rules_dx//android/rules:defs.bzl`: Android libraries.
- `@rules_dx//api_spec/rules:defs.bzl`: OpenAPI and AsyncAPI specs.
- `@rules_dx//apple/ios:app.bzl`: iOS apps, tests and frameworks.
- `@rules_dx//apple/ios:defs.bzl`: iOS libraries.
- `@rules_dx//cc/rules:defs.bzl`: C and C++.
- `@rules_dx//csharp/rules:defs.bzl`: C#.
- `@rules_dx//cuda/rules:defs.bzl`: CUDA.
- `@rules_dx//cue/rules:defs.bzl`: CUE.
- `@rules_dx//dockerfile/rules:defs.bzl`: Dockerfiles.
- `@rules_dx//d/rules:defs.bzl`: D.
- `@rules_dx//elixir/rules:defs.bzl`: Elixir.
- `@rules_dx//erlang/rules:defs.bzl`: Erlang.
- `@rules_dx//fsharp/rules:defs.bzl`: F#.
- `@rules_dx//gherkin/rules:defs.bzl`: Gherkin.
- `@rules_dx//gleam/rules:defs.bzl`: Gleam.
- `@rules_dx//go/rules:defs.bzl`: Go.
- `@rules_dx//haskell/rules:defs.bzl`: Haskell.
- `@rules_dx//html_template/rules:defs.bzl`: HTML templates.
- `@rules_dx//java/rules:defs.bzl`: Java.
- `@rules_dx//javascript/rules:defs.bzl`: JavaScript.
- `@rules_dx//javascript/rules:web.bzl`: web apps and dev servers.
- `@rules_dx//jsonnet/rules:defs.bzl`: Jsonnet.
- `@rules_dx//kotlin/rules:defs.bzl`: Kotlin.
- `@rules_dx//lua/rules:defs.bzl`: Lua.
- `@rules_dx//perl/rules:defs.bzl`: Perl.
- `@rules_dx//pkl/rules:defs.bzl`: Pkl.
- `@rules_dx//php/rules:defs.bzl`: PHP.
- `@rules_dx//powershell/rules:defs.bzl`: PowerShell.
- `@rules_dx//python/rules:defs.bzl`: Python.
- `@rules_dx//qt/rules:defs.bzl`: Qt.
- `@rules_dx//qt/rules:rust.bzl`: Rust libraries for Qt.
- `@rules_dx//ruby/rules:defs.bzl`: Ruby.
- `@rules_dx//rust/rules:defs.bzl`: Rust.
- `@rules_dx//scala/rules:defs.bzl`: Scala.
- `@rules_dx//sql/rules:defs.bzl`: SQL.
- `@rules_dx//terraform/rules:defs.bzl`: Terraform.
- `@rules_dx//typescript/rules:defs.bzl`: TypeScript.
- `@rules_dx//vue/rules:defs.bzl`: Vue.
- `@rules_dx//xml/rules:defs.bzl`: XML.
- `@rules_dx//yaml/rules:defs.bzl`: YAML.
- `@rules_dx//zig/rules:defs.bzl`: Zig.

Packaging and deploy:

- `@rules_dx//android/packaging:defs.bzl`: APKs, App Bundles and adb.
- `@rules_dx//appimage:defs.bzl`: AppImage bundles.
- `@rules_dx//apple/ios:device.bzl`: signed iOS IPA export.
- `@rules_dx//apple/provisioning:defs.bzl`: Apple provisioning.
- `@rules_dx//apple/provisioning:probe.bzl`: Apple toolchain probe.
- `@rules_dx//apple/signing:defs.bzl`: macOS app signing and notarization.
- `@rules_dx//deploy/offline:offline.bzl`: offline bundles.
- `@rules_dx//deploy/rules:archive.bzl`: release archives.
- `@rules_dx//deploy/rules:cloudflare.bzl`: Cloudflare deploys.
- `@rules_dx//deploy/rules:crates.bzl`: crates.io publish.
- `@rules_dx//deploy/rules:github.bzl`: GitHub releases.
- `@rules_dx//deploy/rules:maven.bzl`: Maven publish.
- `@rules_dx//deploy/rules:npm.bzl`: npm publish.
- `@rules_dx//deploy/rules:nuget.bzl`: NuGet publish.
- `@rules_dx//deploy/rules:oci.bzl`: OCI images.
- `@rules_dx//deploy/rules:octopus.bzl`: Octopus Deploy.
- `@rules_dx//deploy/rules:promotion.bzl`: release promotion.
- `@rules_dx//deploy/rules:pypi.bzl`: PyPI publish.
- `@rules_dx//msix:defs.bzl`: MSIX packages.

SDK and toolchain extensions:

- `@rules_dx//android/sdk:repos.bzl`: Android SDK.
- `@rules_dx//android/toolchain:repos.bzl`: Android NDK toolchain.
- `@rules_dx//apple/ios:simulator.bzl`: iOS simulator.
- `@rules_dx//cuda/toolchain:repos.bzl`: CUDA license acceptance.
- `@rules_dx//elixir/rules:hex.bzl`: Hex packages for Elixir.
- `@rules_dx//gleam/rules:hex.bzl`: Hex packages for Gleam.
- `@rules_dx//powershell/rules:gallery.bzl`: PowerShell Gallery modules.
- `@rules_dx//msix:repos.bzl`: MSIX tools.
- `@rules_dx//qt/sdk:repos.bzl`: Qt SDK.

API docs:

- `@rules_dx//tools/docs/go:defs.bzl`: Go API doc shards.
- `@rules_dx//tools/docs/ruby:defs.bzl`: Ruby API doc shards.
- `@rules_dx//tools/docs/powershell:defs.bzl`: PowerShell API doc shards.
- `@rules_dx//tools/docs/rust:defs.bzl`: Rust API doc shards.
- `@rules_dx//tools/docs/site:site.bzl`: docs sites built from API doc shards.
- `@rules_dx//tools/docs/typescript:defs.bzl`: TypeScript API doc shards.

Quality config:

- `@rules_dx//quality:native_config.bzl`: native linter and formatter configs.
- `@rules_dx//quality:policy.bzl`: quality families and workspace policy.

Every other file stays private.
A private file is not a dependency.
Patch releases keep every supported label and flag.
Minor releases add without breaking old uses.
A breaking change needs a new major version and a migration.
Readers tolerate added fields and fail on a new major schema.
Bazel 9.2.0 is the tested baseline.

rules_dx does not provide debugger acquisition, launch or attach integration,
Debug Adapter Protocol adapters, or generated debugger configurations. These
are unsupported and not planned. Debug build profiles and editor
language-server settings do not imply debugger support. You can set up
external debuggers yourself.
