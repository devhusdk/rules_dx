# Build, Test, And Coverage Commands

```sh
bazel run @rules_dx//:dx -- build //...
bazel run @rules_dx//:dx -- test //...
bazel run @rules_dx//:dx -- coverage //...
```

No scope means `//...`. Pass a label, pattern, file, or dir to narrow it.
Use `--here` for the current dir tree, or `--affected` for the targets your
git changes reach (see [Scope Defaults](scope-defaults.md)). File scopes
resolve to the owning targets. For `dx test` and `dx coverage`, a file runs
the tests that depend on its owners. A test tagged `manual` runs only when
it owns the file. Args after `--` go to Bazel unchanged.
`dx build`, `dx test`, `dx coverage`, `dx run` and `dx deploy` also pass
`--@rules_dx//config:validate=false`, `--@rules_dx//config:clippy_output_diagnostics=true`
and `--@rules_dx//config:rustc_output_diagnostics=true`. Another value for one of them,
or for the matching `@rules_rust//rust/settings` flag, fails before Bazel starts with exit `2`.
dx spells them with the name your MODULE.bazel gives rules_dx, for example
`--@my_dx//config:validate=false` when `repo_name = "my_dx"`.

Ctrl-C, `SIGTERM` or `SIGHUP` stops a run. A `SIGHUP` that was ignored when
dx started, as under `nohup`, stays ignored. Ctrl-C, and a signal sent to dx itself,
even from outside a container, reach Bazel or the app once.
On macOS in a terminal, a `SIGINT` or `SIGHUP` sent to dx alone is not passed on,
and dx stops once Bazel or the app ends; send `SIGTERM` to stop it right away.
Outside a terminal, every process dx started gets it too, and any still running after
10 seconds is killed. A stopped run exits by that signal and emits no
`command_finished` event. The shared Bazel server keeps running. If Bazel
dies by a signal dx did not get, dx reports a `bazel_signalled` error and
exits `1`.

Outside a terminal, if dx is killed with `SIGKILL`, Linux stops the process dx
started. Anything else in that process group keeps running until the next dx
run in that worktree stops it, prints `dx: stopped leftover process group <id>
from dx run <pid>, which is gone`, and with `--output=json` emits a `notice`
event with code `orphan_process_group_stopped`, after `command_started`. On
macOS the next run stops the whole tree. In a terminal, a `SIGKILL` to dx alone
leaves its children running. Outside a terminal, when the process dx started
exits, dx stops anything still running in its process group. A dx that exits
on its own never leaves anything for the next run to stop.

## `dx build` And `dx test`

```text
dx build [--here] [--affected] [--debug|--release] [--report failures=<path>] [scope...] [-- bazel-options...]
dx test [--here] [--affected] [--debug|--release] [--report junit=<path>] [--report failures=<path>] [--report results=<path>] [scope...] [-- bazel-options...]
```

Builds or tests the scope. `--debug` uses the `dx_debug` profile,
`--release` uses `dx_release`. No flag uses `dx_dev`. The two flags conflict. `--debug` changes compilation
only. It does not start or attach a debugger. See
[Supported Surface](README.md#supported-surface).
Exec tools build at `fastbuild`; `--release` builds them at `opt`.
Extra `--config` values pass through after the dx profile in their original
order. Only a conflicting dx profile is rejected.
dx adds its own report flags after your Bazel options, so a `--config` cannot
change them. `dx test` and `dx coverage` reject `--remote_download_minimal`,
`--remote_download_toplevel` and any `--remote_download_outputs` other than
`all`. `dx coverage` also rejects any `--combined_report` other than `lcov`.
dx writes its build event file with
`--build_event_json_file_path_conversion=false`, so it rejects
`--build_event_json_file` and any other `--build_event_json_file_path_conversion`
value. A rejected flag fails before Bazel starts with exit `2`.

Output: `--output text|json`. Reports: `dx test` writes
`--report junit=<path>` JUnit reports. Repeat the flag for more files. Use `-`
for stdout. A relative path anchors at the workspace root; an absolute one writes
as given. Colliding destinations fail before anything runs, parents must exist,
and a failed write keeps the previous file bytes. JUnit case names gain
`[run=N]` or `[config=<id>]` when a target has several runs or configurations,
and `[shard=N,attempt=N]` for later shards and attempts. A `test.xml` with
invalid UTF-8 still counts; dx replaces the bad bytes and prints a
`dx: warning:` on stderr. `dx test --output=json` adds one
`test_outcome` event per test result except a plain pass (`passed` or
`no_cases` with complete evidence), with its Bazel status,
run, shard, attempt, cached state, duration, and case counts. A failing result
also carries `failing_cases` (up to 10, each with `classname`, `name`,
`outcome` and `message`), `log_path`, and `log_tail` (the last 40 lines of
`test.log`). `artifacts` and `evidence_complete` appear only when evidence is
missing. `final` is `false` on an attempt that a later attempt of the same
target, configuration, run and shard replaced, and appears only then. Plain
passes are not events; `passed_after_retry` is. When dx collected any test result,
`command_finished` has `tests` with `passed` (every result that passed,
including `passed_after_retry` and `no_cases`, matching the passed entries in
the `results` report), `failed` (results whose final attempt has outcome
`failed` or `timeout`), `cached` (passed results that came from the
cache), and `no_cases` (passed results whose report had no test cases; part of
`passed`), and `skipped` (tests with outcome `skipped` or `filtered`). Earlier
attempts of a retried test still appear as `test_outcome` events with
`final: false` and are not counted.

A test that Bazel skipped has outcome `skipped`. A test that a tag or size
filter removed from the run has outcome `filtered`. Neither changes the exit
code. When any test was filtered, dx emits one `notice` event with code
`tests_filtered` that gives the count and the first labels. In text mode it
prints `dx: tests_filtered: ...` on stderr.

`--report results=<path>` writes one JSON file with every test result, passed
or not. It follows the same path rules as the other reports. With
`--output=json` and no `results` report, dx writes it to
`.dx/results.json` in the workspace and emits a `report`
event with the path. Such a run removes the old `.dx/results.json` first, even
when it stops before Bazel starts. Text runs and runs with a `results` report
leave it alone.
The file has:

- `record_version`: `1`, `exit_code`: Bazel's code.
- `complete`: `false` when results are incomplete.
- `results`: one entry per test result, with the `test_outcome` fields, every
  case as `classname`, `name`, `outcome` and `message`, and `log_path` when
  the result did not pass. A replaced attempt carries `final: false`.

`dx build`, `dx test` and `dx coverage` report each failure Bazel records
when it exits nonzero. `--output=text` prints one `dx: build failure:` line
per failure on stderr, with its rustc diagnostics under it. Each diagnostic
lists its labels, notes and helps indented below it.
`--output=json` adds one `build_failure` event per failure, before
`command_finished`. The same failure from several targets or configurations is
one event with a `count` and up to 20 other `targets`. Each event has:

- `record_version`: `1`. New fields are only ever added.
- `target`, `configuration`, `target_kind` (`test`, `other` or `unknown`).
- `stage`: `analysis`, `tool_acquisition`, `compilation`, `linking`,
  `test_execution`, `other` or `unknown`.
- `count` and `targets`: only when the failure repeats.
- `category`: `analysis_failed`, `loading_failed`, `tool_unavailable`,
  `tool_missing`, `compile_failed`, `link_failed`, `test_execution_failed`,
  `action_failed`, `target_not_built`, `skipped`, `cancelled` or `aborted`.
- `action`: `mnemonic`, `primary_output` and, when known, `exit_code`.
- `tool`: `clippy` for `Clippy` actions and `rustc` for other Rust actions.
- `message`: the first line of Bazel's reason, for failures with no action.
- `diagnostics`: `status`, `total` and up to 50 `items` with `severity`,
  `code`, `message`, `path`, `line`, `column` and an ordered `related` list.
  Each `related` item has `role` (`label`, `note` or `help`), `message`, and
  when known `path`, `line`, `column`, `replacement` and `applicability`.
  Labels read from rendered output have a `line` but no `column`. Related
  items are not counted in `total`. Link errors have none.

Anything Bazel did not report reads `unknown`. An analysis failure has no
action, so its action fields are `unknown`. A test target that fails to build
emits a `build_failure` with `target_kind` `test` and no test case in the JUnit
report. It also gets a `test_outcome` with outcome `failed_to_build`, no case
counts, and `status` `FAILED_TO_BUILD` only when Bazel reports that summary.
The run's results are incomplete. Environment variables and command lines are
never reported.

Diagnostics come from rustc's JSON or rendered output in the failed action's
stderr. `status` is `parsed` when diagnostics were read, `none` when the stderr
held none, `unavailable` when the stderr is unreadable or named only on a remote
cache, and `unknown`
for non-Rust actions. A Rust action whose diagnostics include a `linking with`
error is reported as `linking`.

The failures report: `--report failures=<path>` on `dx build`, `dx test` and `dx coverage` writes
one JSON file with the failures of this run. It follows the same path rules
as the other reports and may share a run with `junit` or `lcov`. Use `-` for
stdout in text mode. The file is written on success too, with an empty
`failures` list. A failed write is a `report_failed` error. It fails a run Bazel
passed and never changes Bazel's own code.

The file has:

- `record_version`: `1`, `exit_code`: Bazel's code.
- `complete`: `false` when the build events end early or cannot be read, with
  the reason in `problem`.
- `truncated`: `true` when any stderr was cut at a limit.
- `failures_omitted`: failures past the first 200.
- `failures`: the `build_failure` fields above, each with a `stderr` object.

`stderr` has `status`, `bytes`, `truncated`, `redacted_lines` and, for
captured output, `text`. `status` is:

- `structured` for Rust actions whose diagnostics were parsed. Read
  `diagnostics` instead.
- `captured` or `truncated` for other actions. Each failure keeps at most
  64 KiB and the file at most 1 MiB. Later failures read `omitted_limit`.
- `remote` when the build events still name the stderr only on a remote cache.
  dx asks Bazel to name local stderr files, so this is rare.
- `absent` when Bazel named no stderr, `unreadable` when the local file is
  gone.

Only this run's failed actions are read. No directory is scanned and no
history is kept. Lines that start a command line (`(cd ` or `exec env`) are
dropped and counted in `redacted_lines`. Environment variables and command
lines are never reported. Positions come only from rustc diagnostics.

```sh
bazel run @rules_dx//:dx -- build --report failures=build-failures.json //...
bazel run @rules_dx//:dx -- test --report junit=junit.xml --report failures=failures.json
bazel run @rules_dx//:dx -- test --report results=results.json //...
```

In an agent session (`CLAUDECODE`, `CODEX_THREAD_ID`, `CURSOR_AGENT`,
`GEMINI_CLI` or `AI_AGENT` set), `dx build`, `dx test`, `dx coverage` and
`dx run` default to `--output=json`. They pass `--noshow_progress`,
`--noshow_loading_progress`, `--noshow_timestamps`, `--curses=no`,
`--ui_event_filters=-info,-debug` and `--test_summary=none` to Bazel before
your own Bazel options, so Bazel prints no progress on stderr. For `dx run`,
args after `--` still go only to the app. `--output` and `DX_OUTPUT` still
win. In CI these variables are ignored.

If Bazel fails and its event file is missing or ends early, dx still returns
Bazel's code and reports a `build_events_incomplete` notice. `dx test` and
`dx coverage` write the reports they can, and the results report has
`complete: false`. If Bazel succeeds and `dx test` or `dx coverage` cannot
read the event file, dx exits `1` with `unreadable_bep` or `invalid_bep`.

Exit codes: `0` success. `2` usage or scope errors. `1` operational
failures. Bazel failures keep Bazel's code. Results are incomplete when a
result has no `test.xml`, a `test.xml` is missing, truncated or invalid,
Bazel announces a result it never reports, a target has a summary but no
result, or the build events end before Bazel's last event. Another test's
evidence never makes up for it. Incomplete results fail the run, even when
Bazel passes. `--output=text` prints `dx: incomplete_results: <reason>` on
stderr, `--output=json` emits an `incomplete_results` error, the JUnit report
gets a `dx.infrastructure` error case, and `results_complete` reads `false`.
Failing tests with every result collected keep Bazel's code `3` and read
`results_complete: true`. Other Bazel failures read `false`.
With exit `3` and at least one failed result, `--output=json` emits a
`tests_failed` error with cause `test` and Bazel's status. Other Bazel failures
emit `bazel_failed`. `dx coverage` does the same.

```sh
bazel run @rules_dx//:dx -- build //cli/...
bazel run @rules_dx//:dx -- test --here
bazel run @rules_dx//:dx -- test --affected
bazel run @rules_dx//:dx -- test //... -- --jobs=4
bazel run @rules_dx//:dx -- test //cli/process/... -- --test_filter=workflow_argv
```

Args after `--` reach the test binary through Bazel. `dx test` and
`dx coverage` accept repeatable `--test_arg` tokens, in equals and
two-token form, with spaces and Unicode kept verbatim. `--test_filter`
stays a Bazel option. When the selected tests are `rust_test` targets,
optionally with their `rustfmt_test` and `rust_clippy_test` checks, dx also
passes the last `--test_filter` value to the test binaries as a libtest
filter, so only matching cases run. Other commands
reject `--test_arg`; run those cases with `bazel` directly.

Tests run once by default. Only tests marked flaky in their build
definition retry, up to three attempts. Diagnose one test with retries
through `dx test <label> -- --flaky_test_attempts=3`, or through
`bazel test --config=retry <label>`. A test that passes after retry is
reported as `passed_after_retry` with its attempt number.

## `dx run`

```text
dx run [--debug|--release] <label...> [-- args...]
```

Builds the runnable targets in scope, then launches them in scope
order. Explicit labels and patterns run sequentially. File and directory
scopes must resolve to exactly one runnable target. Args after `--` go to
the app. The default profile is development; `--release` selects release.
`dx run` works with `CI=true`. `dx run --apply` is a usage error.
The build holds a [build slot](README.md#build-slots). The slot frees before
the app starts.

Output: `--output text|json`. `dx run` has no report format. With
`--output=json`, an app that exits nonzero emits an `error` with code
`application_failed`, cause `application`, the app's exit code as `status`,
and `correlation` `run:<label>`.
A failed build emits an `error` with code `bazel_failed` and starts no app.

Exit codes: `0` success, `2` usage or scope errors, `1` no runnable target or
a launch failure. A failed build exits with Bazel's code. The app's own code
is the exit code.

```sh
bazel run @rules_dx//:dx -- run @rules_dx//:dx -- --help
bazel run @rules_dx//:dx -- run --release @rules_dx//:dx -- --help
```

## `dx deploy`

```text
dx deploy [--apply] [--debug|--release] <label> [-- args...]
```

Builds one deployable target without publishing it. Pass `--apply` to run
the deployment. Takes exactly one label. No flag uses `dx_release` here.
Args after `--` go to the app, and only reach it on an `--apply` launch.

Output: text only.

Exit codes: `0` success, `2` usage or scope errors, `1` a build or launch
failure. The target's own code is the exit code. A passing check names
the `--apply` command that publishes.

`--affected` builds or tests the changed targets and their reverse
dependencies. A `Cargo.lock` or crate_universe lockfile change selects the
users of the changed crates. Other lock files and patches named in
`MODULE.bazel` widen to `//...`; see [scope defaults](scope-defaults.md).
`dx test --affected` and `dx coverage --affected` run only the
affected test targets. With nothing affected they print `no affected targets`
and exit `0`. Usage errors exit `2`. Affected targets that do not fit the host
platform are skipped; see [Scope Defaults](scope-defaults.md).

## `dx coverage`

```text
dx coverage [--here] [--affected] [--min-coverage <percent>] [--report lcov=<path>] [--report failures=<path>] [--report results=<path>] [scope...] [-- bazel-options...]
```

Collects LCOV coverage over the scope. `--min-coverage` fails below that
percent. Without it, coverage collects without enforcing.

Output: `--output text|json`. Reports: `--report lcov=<path>` writes
combined LCOV. Repeat the flag for more files. Use `-` for stdout. A relative
path anchors at the workspace root; an absolute one writes as given. Colliding
destinations fail before anything runs, parents must exist, and a failed write
keeps the previous file bytes.
`dx coverage --output=json` adds one `test_outcome` event per test result except
a plain pass, as for `dx test`. `command_finished` counts tests as for `dx test`.

`--report failures=<path>` and `--report results=<path>` work here too, as
described above.

Exit codes: `0` success. `2` usage or scope errors. `1` operational
failures, coverage below minimum, or incomplete coverage. Bazel failures
keep Bazel's code. Coverage is incomplete when Bazel reports a
`coverage.dat` artifact that is missing or invalid, when no coverage
artifacts are reported, or when the build events are incomplete as for
`dx test`. Incomplete coverage fails the run, blocks
`--min-coverage`, and marks a written LCOV report incomplete. A retried
test counts only its final attempt. Tests without coverage outputs are
not expected to produce coverage. Empty but valid `coverage.dat` files
are accepted and contribute no lines.

```sh
bazel run @rules_dx//:dx -- coverage //...
bazel run @rules_dx//:dx -- coverage --min-coverage 100 //...
```

Coverage ignores use `LCOV_EXCL_LINE` or `LCOV_EXCL_START` / `LCOV_EXCL_STOP`
with a short `reason:` and `issue:`. Bare ignores without a reason fail the
gate.

The comment syntax depends on the language: `//` for Rust, Go, JS/TS, the JVM
languages (including Kotlin scripts, `.kts`), .NET and C/C++; `#` for Python
and Ruby; `%` for Erlang (`.erl` and `.hrl`). Markers in other sources
(`.bzl`, `.pyi`, `.cu`/`.cuh`, `.d.ts`) are not read.

## Line And Branch Metrics

The coverage gate enforces exact executable-line coverage. Branch outcomes
are measured separately and never change the gate. Each file with branch
records reports taken and recorded counts with its language. Files without
branch records report unavailable instead of a rate. A declared branch
summary that does not match its records fails the report as incomplete.
