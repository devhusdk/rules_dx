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
`--@rules_dx//config:clippy_output_diagnostics=true`
and `--@rules_dx//config:rustc_output_diagnostics=true`. Another value for one of them,
or for the matching `@rules_rust//rust/settings` flag, fails before Bazel starts with exit `2`.
dx spells them with the name your MODULE.bazel gives rules_dx, for example
`--@my_dx//config:clippy_output_diagnostics=true` when `repo_name = "my_dx"`.

Ctrl-C, `SIGTERM` or `SIGHUP` stops a run. A `SIGHUP` that was ignored when
dx started, as under `nohup`, stays ignored. Ctrl-C, a signal sent to dx, and a signal
sent to dx's process group each reach Bazel or the app once, even from outside a container.
In a terminal, Bazel or the app runs in its own foreground process group; Ctrl-Z stops it and dx, and `fg` resumes both.
Outside a terminal, every process dx started gets it too, and any still running after
10 seconds is killed. A stopped run prints `dx: cancelled: stopped by signal N`, exits `128` plus the signal number, and emits no
`command_finished` event. The shared Bazel server keeps running. If Bazel
dies by a signal dx did not get, dx reports a `bazel_signalled` error and
exits `1`. Under `dx run` and `dx deploy --apply`, an app that dies by its own
signal is an app failure instead: dx prints
`dx: application_failed: bazel run <label> terminated by signal N` and exits
`128` plus the signal number.

Outside a terminal, if dx is killed with `SIGKILL`, Linux stops the process dx
started. Anything else in that process group keeps running until the next dx
run in that worktree stops it, prints `dx: stopped leftover process group <id>
from dx run <pid>, which is gone`, and with `--output=json` emits a `notice`
event with code `orphan_process_group_stopped`, after `command_started`. dx
stops a leftover group only when it can confirm it is the group it recorded;
otherwise it leaves the group running. Each stop is logged to
`.dx/leases/process-groups/reaped.log`. On macOS the next run stops the whole
tree the same way. In a terminal, a `SIGKILL` to dx alone
leaves its children running. Outside a terminal, when the process dx started
exits, dx stops anything still running in its process group. A dx that exits
on its own never leaves anything for the next run to stop.

## `dx build` And `dx test`

```text
dx build [--here] [--affected] [--debug|--release] [--explain] [--report failures=<path>] [scope...] [-- bazel-options...]
dx test [--here] [--affected] [--debug|--release] [--explain] [--report junit=<path>] [--report failures=<path>] [--report results=<path>] [scope...] [-- bazel-options...]
```

Builds or tests the scope. `--debug` uses the `dx_debug` profile,
`--release` uses `dx_release`. No flag uses `dx_dev`. The two flags conflict. `--debug` changes compilation
only. It does not start or attach a debugger. See
[Supported Surface](README.md#supported-surface).
Exec tools build at `fastbuild`; `--release` builds them at `opt`.
When Bazel has no output base for the workspace yet, `dx build`, `dx test`,
`dx coverage`, `dx run` and `dx deploy` print one line on stderr before Bazel
starts. `--quiet` and `--output=json` drop it:

```text
dx: first run in this workspace; Bazel will fetch toolchains and dependencies and this can take several minutes
```

`--explain` saves the Bazel execution log under `.dx/explain/` for
[`dx explain`](explain.md) and keeps the two newest logs. JSON adds a
`notice` (`explain_log`) with the log path; text prints
`dx: execution log <path>` on stderr. With `--explain`, Bazel options that
set another execution log are refused.
Extra `--config` values pass through after the dx profile in their original
order. Only a conflicting dx profile is rejected.
dx adds its own report flags after your Bazel options, so a `--config` cannot
change them. `dx test` and `dx coverage` add `--remote_download_outputs=toplevel`
and reject `--remote_download_minimal` and any `--remote_download_outputs` other
than `toplevel` or `all`. Pass `--remote_download_outputs=all` or
`--remote_download_all` to download all outputs instead; the last of these wins. `dx coverage` also rejects any `--combined_report` other than `lcov`.
dx writes its build event file with
`--build_event_json_file_path_conversion=false`, so it rejects
`--build_event_json_file` and any other `--build_event_json_file_path_conversion`
value. A rejected flag fails before Bazel starts with exit `2`.

dx forces these flags and treats them as protected: `--skip_incompatible_explicit_targets`
(build, test and coverage), the three `--@rules_dx//config:` settings for validation and
rustc and clippy diagnostics, `--remote_download_outputs=toplevel` (test and coverage, or `all` if you pass it),
`--combined_report=lcov` (coverage), and `--build_event_json_file`. Another value
for one of them, and Bazel startup options after `--`, exit `2`.

Output: `--output text|json`. `command_finished` carries `actions`, the
Bazel action counts of the build (see the
[JSON output](README.md#json-output) list). Text output ends with
`dx: <total> actions, <executed> executed, <cached> cached` on stderr, where
`<total>` is executed plus cached. `--quiet` drops that line. Reports: `dx test` writes
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
`outcome` and `message`), `log_path`, and `log_tail` (the last 40 lines, at most 8 KiB of
`test.log`). `artifacts` and `evidence_complete` appear only when evidence is
missing. `final` is `false` on an attempt that a later attempt of the same
target, configuration, run and shard replaced, and appears only then. Plain
passes are not events; `passed_after_retry` is. When dx collected any test result,
`command_finished` has `tests`, counted once per result (per target,
configuration, run and shard, so a target with four shards counts four), with `passed` (every result that passed,
including `passed_after_retry` and `no_cases`, matching the passed entries in
the `results` report), `failed` (results whose final attempt has outcome
`failed` or `timeout`), `cached` (passed results that came from the
cache), and `no_cases` (passed results whose report had no test cases; part of
`passed`), and `skipped` (tests with outcome `skipped` or `filtered`). Earlier
attempts of a retried test still appear as `test_outcome` events with
`final: false` and are not counted.

A test that Bazel skipped has outcome `skipped`. A test that a tag or size
filter removed from the run has outcome `filtered`. Neither changes how the
run ends. When any test was filtered, dx emits one `notice` event with code
`tests_filtered` that gives the count and the first labels. In text mode it
prints `dx: tests_filtered: ...` on stderr.

`--report results=<path>` writes one JSON file with every test result, passed
or not. It follows the same path rules as the other reports. With
`--output=json` and no `results` report, dx writes it to
`.dx/results.json` in the workspace and emits a `report`
event with the path. A failed write of that default file fails the run, like a failed explicit
`--report results=<path>`: dx keeps Bazel's nonzero code or else exits nonzero,
`results_complete` is `false`, and a `report_failed` error is emitted. To write
the file somewhere else, pass `--report results=<path>`. Such a run removes the old `.dx/results.json` first, even
when it stops before Bazel starts. Text runs and runs with a `results` report
leave it alone.
The file has:

- `record_version`: `1`, `exit_code`: Bazel's code.
- `run_id`: identifies the run that wrote the file; `command_finished` carries
  the same `run_id`. Concurrent runs share `.dx/results.json`, so check that
  `run_id` matches `command_finished.run_id`, or give each run its own path
  with `--report results=<path>`.
- `complete`: `false` when results are incomplete.
- `results`: one entry per test result, with the `test_outcome` fields, every
  case as `classname`, `name`, `outcome` and `message`, and `log_path` when
  the result did not pass. A replaced attempt carries `final: false`.

`dx build`, `dx test` and `dx coverage` report each failure Bazel records
when it exits nonzero. `--output=text` prints one `dx: build failure:` line
per failure on stderr. A repeated failure lists its target and up to twenty others
in that line, then `xN`, the total across targets and configurations. Its rustc
diagnostics follow it. Each diagnostic lists its labels, notes and helps indented below it.
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
your own Bazel options, so Bazel prints no progress on stderr. Those quiet
Bazel flags apply only when the output is JSON. For `dx run`, they come
before the `--bazel-option` values, and args after `--` still go only to the app. `--output`, `DX_OUTPUT` and the `output` setting in `dx.toml` still
win. In CI these variables are ignored.

If Bazel fails and its event file is missing or ends early, dx still returns
Bazel's code and reports a `build_events_incomplete` notice. `dx test` and
`dx coverage` write the reports they can, and the results report has
`complete: false`. If Bazel succeeds and `dx test` or `dx coverage` cannot
read the event file, dx exits `1` with `unreadable_bep` or `invalid_bep`.

Exit codes: `0` success. `2` usage, scope or owner errors. `1` operational
failures, including `no_tests` and `query_launch_failed`. Bazel failures keep Bazel's code. Results are incomplete when a
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

`--affected` builds or tests the changed targets and their reverse
dependencies. A `Cargo.lock` or crate_universe lockfile change selects the
users of the changed crates. A `bazel_dep` version bump in the root
`MODULE.bazel` selects the users of the changed modules. Other lock files and patches named in
`MODULE.bazel` widen to `//...`; see [scope defaults](scope-defaults.md).
`dx test --affected` and `dx coverage --affected` run only the
affected test targets. With nothing affected they print `no affected targets`
and exit `0`. Usage errors exit `2`. Affected targets that do not fit the host
platform are skipped; see [Scope Defaults](scope-defaults.md).

Args after `--` reach the test binary through Bazel. `dx test` and
`dx coverage` accept repeatable `--test_arg` tokens, in equals and
two-token form, with spaces and Unicode kept verbatim. `--test_filter`
stays a Bazel option. When every selected test is a `rust_test` that uses
the libtest harness, optionally with its `rustfmt_test` and
`rust_clippy_test` checks, dx also passes the last `--test_filter` value to
the test binaries as a libtest filter, so only matching cases run. To decide,
dx first queries the kinds of the selected tests with the run's repository
options, such as `--override_repository` and `--repo_env`. If that query
fails, dx stops with a query error before Bazel runs. Other commands
reject `--test_arg`; run those cases with `bazel` directly.

Tests run once by default. Only tests marked flaky in their build
definition retry, up to three attempts. Diagnose one test with retries
through `dx test <label> -- --flaky_test_attempts=3`, or through
`bazel test --config=retry <label>`. A test that passes after retry is
reported as `passed_after_retry` with its attempt number.

## `dx run`

```text
dx run [--debug|--release] [--bazel-option <token>]... <label...> [-- args...]
```

Builds the runnable targets in scope, then launches them in scope
order. Explicit labels and patterns run sequentially. File and directory
scopes must resolve to exactly one runnable target. Args after `--` go to
the app. `--bazel-option <token>` passes one Bazel option to both the build
and the run, and is repeatable. Bazel startup options and `--test_arg` are
usage errors there. For example,
`dx run --bazel-option=--jobs=4 //app:bin -- --port=8080`.
The default profile is development; `--release` selects release.
`dx run` works with `CI=true`. `dx run --apply` is a usage error.
The build holds a [build slot](README.md#build-slots). The slot frees before
the app starts.

Output: `--output text|json`. `dx run` has no report format. With
`--output=json`, an app that exits nonzero emits an `error` with code
`application_failed`, cause `application`, the app's exit code as `status`,
and `correlation` `run:<label>`. An app that dies by a signal emits the same
`error`, with its signal exit code from the list below as `status`.
A failed build emits an `error` with code `bazel_failed` and starts no app.

Exit codes: `0` success, `2` usage or scope errors, `1` no runnable target or
a launch failure, or a file or directory scope that matches more than one runnable
target (ambiguous). A failed build keeps Bazel's exit code. The app's own code
is the exit code, and an app that dies by a signal exits `128` plus the signal
number. The first app that exits nonzero or dies by a signal stops the later targets.

```sh
bazel run @rules_dx//:dx -- run @rules_dx//:dx -- --help
bazel run @rules_dx//:dx -- run --release @rules_dx//:dx -- --help
```

## `dx deploy`

```text
dx deploy [--apply] [--debug|--release] <label> [-- args...]
```

Builds one deployable target without publishing it. Pass `--apply` to run
the deployment. Takes exactly one label. The profile is the flag if given, else the target's
`profile` attribute, else the command default. With `--apply`, dx sets
`DX_PROFILE` for the deploy tool.
Args after `--` go to the app, and only reach it on an `--apply` launch.

Output: text only.

Exit codes: `0` success, `2` usage or scope errors, `1` a build or launch
failure. The target's own code is the exit code, and a target that dies by a
signal exits `128` plus the signal number. A passing check names
the `--apply` command that publishes.

## `dx coverage`

```text
dx coverage [--here] [--affected] [--explain] [--min-coverage <percent>] [--min-changed-line-coverage <percent>] [--report lcov=<path>] [--report sources=<path>] [--report failures=<path>] [--report results=<path>] [scope...] [-- bazel-options...]
```

Collects LCOV coverage over the scope. `--min-coverage` takes a whole number
from 0 to 100 and fails below that percent. The gate counts the cell's owned
sources (see [Coverage sources](#coverage-sources)): the denominator is every
eligible line of every eligible production source, after valid exclusions, and
the numerator is the eligible lines with hits. `100` means covered equals
eligible. Lines in unsupported languages are not counted. Every coverage
source gap also fails the gate: a production source absent from the LCOV, an
LCOV path no target owns, an invalid source manifest, or invalid exclusion
markers. To clear an absent source, test it or exclude the whole file with an
`LCOV_EXCL_START` / `LCOV_EXCL_STOP` range. Without the flag, coverage collects
without enforcing. A passing gate prints
`coverage <percent>% (<covered>/<eligible> lines) meets minimum <n>% in cell <cell>`.
A failing one prints `coverage_below_minimum: coverage <percent>% (<covered>/<eligible> lines) below minimum <n>% in cell <cell>`,
or `coverage_below_minimum: coverage <percent>% (<covered>/<eligible> lines) in cell <cell> has <n> coverage source gaps (see coverage_sources_incomplete); minimum <n>%`
on stderr, or emits an `error` event with code `coverage_below_minimum` with
`--output=json`. A cell with zero eligible lines also fails. A
`--instrumentation_filter` narrower than the preset shrinks the source list
without a warning.
C and C++ tests report line coverage through LLVM, with no setup needed.

`--min-changed-line-coverage` takes a whole number from 0 to 100 and gates
only the lines the working tree adds since the merge-base of `HEAD` with the
default branch (`origin/HEAD`, else `main`, else `master`). Untracked files
count in full; `.dx/` is skipped. Renamed files count only their edited lines.
The denominator is every added line the LCOV reports in an eligible production
source, after valid exclusion markers; the numerator is those lines with hits.
Test, support, excluded, unsupported, and unowned files are skipped. The gate
needs full Git history. It prints one line on stderr:

```text
dx: changed-line coverage <percent>% (<covered>/<eligible> lines) meets minimum <n>% against merge-base <sha>
dx: changed-line coverage not applicable: no changed executable lines against merge-base <sha>
dx: changed_line_coverage_below_minimum: changed-line coverage <percent>% (<covered>/<eligible> lines) below minimum <n>% against merge-base <sha>
dx: changed_line_uncovered: <path>:<line>
dx: changed_line_coverage_incomplete: changed sources absent from the coverage report against merge-base <sha>: <path>, ...
dx: changed_line_coverage_unavailable: <git error>
```

A pass or not applicable leaves the result unchanged. Below the minimum, a changed
production source the LCOV never saw, or a Git failure (no default branch, a
shallow clone, a failed diff) exits `1`. With `--output=json` dx emits one
`changed_line_coverage` event and, on failure, an `error` event with code
`changed_line_coverage_below_minimum`, `changed_line_coverage_incomplete`, or
`changed_line_coverage_unavailable`.

Not every language has a coverage collector yet:

| Languages | Under `dx coverage` |
| --- | --- |
| Languages with a collector, such as C, C++, Rust, Ruby, C# and F# | Write LCOV |
| Elixir, Zig and PowerShell | Fail with `no_coverage_collector` |

The test macros of the second row tag the public test
`dx-no-coverage-collector`. For each such test that ran, `dx coverage`
prints `dx: no_coverage_collector: no coverage collector for <label>` on
stderr, or emits an `error` event with code `no_coverage_collector` with
`--output=json`, and exits `1`. The coverage results stay complete and a
requested LCOV report is still written.

Output: `--output text|json`. Reports: `--report lcov=<path>` writes
combined LCOV. `--report sources=<path>` writes a JSON record (`record_version`
`2`, `cell`, `candidate` with the `sha` and `dirty` state of the checkout,
`collector`, `owned` with each source's disposition, `absent` production
sources the report never saw, `orphans` tracked sources no rule uses, `errors`); it fails when the evidence is incomplete. Repeat the
flag for more files. Use `-` for stdout. A relative
path anchors at the workspace root; an absolute one writes as given. Colliding
destinations fail before anything runs, parents must exist, and a failed write
keeps the previous file bytes.
`dx coverage --output=json` adds one `test_outcome` event per test result except
a plain pass, as for `dx test`. A failing result carries `failing_cases`,
`log_path` and `log_tail` as in `dx test`; a missing or invalid `test.xml`
there does not make coverage incomplete. `command_finished` counts tests as for
`dx test`.

`--report failures=<path>` and `--report results=<path>` work here too, as
described above.

### Coverage sources

`dx coverage` also lists the production sources each cell owns. The source
list comes from the instrumented files of the targets in scope, so
`--instrumentation_filter` decides which files count. A scoped run counts the
sources of every target in scope, including the transitive production
dependencies of non-test targets in scope. Each source gets one
disposition:

- `eligible`: a production source in a covered language that the LCOV reports
  with executable lines.
- `support`: a test or `testonly` source, a TypeScript declaration file
  (`.d.ts`, `.d.mts`, `.d.cts`), a source the LCOV reports with no executable
  lines, or a C or C++ header the LCOV does not report.
- `excluded`: a source the LCOV does not report, with one valid
  `LCOV_EXCL_START` / `LCOV_EXCL_STOP` range over every line that is not
  blank or a comment.
- `unsupported`: a source in a language the gate does not read. These are
  counted, never dropped.

In text mode dx prints one counts line on stderr. With `--output=json` the
same text is the message of a `notice` event with code `coverage_sources` and
level `info`:

```text
dx: coverage sources <cell>: <raw> raw, <excluded> excluded, <eligible> eligible, <covered> covered lines; <n> eligible, <n> support, <n> excluded, <n> unsupported files
```

`<cell>` is `<os>_<cpu>`, such as `linux_x86_64`. A production source missing
from the LCOV, or an LCOV path no target owns, prints
`dx: coverage_sources_incomplete: <message>` on stderr. So does an invalid
source manifest, manifests from more than one cell, or no manifests at all.
Without `--min-coverage` these are notices: with `--output=json` each is a
`notice` event with code `coverage_sources_incomplete` and level `warning`, and
the exit code does not change. With `--min-coverage` each one fails the run,
even when the line rate meets the minimum, and the verdict line names the gap count, and `--output=json` emits an `error`
event with the same code. A repository that runs `--min-coverage` with an
untested library therefore fails until that library has tests or a whole-file
exclusion.

On a whole-repository run (no scope, or `//...`) dx also checks the tracked
files. It lists them with `git ls-files`, keeps the covered-language files
(not `.d.ts` declarations), and drops the `.dx` state directory, anything
under `.bazelignore`, and files outside any Bazel package. A remaining file
that no rule of the workspace uses as an input prints
`dx: coverage_sources_incomplete: tracked source not owned by any Bazel target: <path>`
and counts as a gap like the others. The rule graph is the unconfigured
`kind('rule', //...)` query, so a file used only on another platform's
`select()` branch, as `data`, or by a `manual` rule is owned. Add the file to a
rule, or delete it. Scoped runs skip this check. Outside a Git work tree dx
prints `dx: tracked sources unavailable: not a Git work tree` and never
enforces it; a Git or query failure inside a work tree is a gap.

With `--output=json`, dx also emits one `coverage` event per run with `cell`,
`covered_lines`, `eligible_lines`, `raw_lines`, `excluded_lines`,
`eligible_files`, `support_files`, `excluded_files`, `unsupported_files`,
`passed`, `errors` (the number of coverage source gaps), `dx_version`, and
`minimum` when `--min-coverage` is set. `sha` (the `HEAD` commit), `dirty`
(true when tracked files differ from it) and `collector` (`bazel <version>`
from the build events) name the checkout and the tool that measured it; they
are left out when unknown. `passed` is true when the cell has no
gaps, has eligible lines, and meets the minimum when one is set. The event is
left out when coverage is incomplete.
`--build_tests_only` is refused, because it hides library targets from the
source list. `dx coverage` also passes `--build_tests_only=false` after your
options, so a `.bazelrc` entry or `--config` that turns it on has no effect.

Exit codes: `0` success. `2` usage, scope or owner errors. `1` operational
failures, coverage below minimum, unaccounted coverage sources with
`--min-coverage`, or incomplete coverage. Bazel failures
keep Bazel's code. Coverage is incomplete when Bazel reports a
`coverage.dat` artifact that is missing or invalid, when no coverage
artifacts are reported, when a target's coverage source list is missing or
unreadable, or when the build events are incomplete as for `dx test`. Incomplete coverage fails the run, blocks
`--min-coverage`, and marks a written LCOV report incomplete. A retried
test counts only its final attempt. Tests without coverage outputs are
not expected to produce coverage. Empty but valid `coverage.dat` files
are accepted and contribute no lines, except from a test tagged
`dx-no-coverage-collector`, which fails the run with `no_coverage_collector`.
To opt out, tag one test `tags = ["no-coverage"]`, or add
`coverage --test_tag_filters=-no-coverage,-dx-no-coverage-collector` to the
repository `.bazelrc`. Keep `-no-coverage` in that line: a
`--test_tag_filters` value replaces the earlier one, and one passed after
`--` replaces the `.bazelrc` line too. Plain `bazel coverage` does not
check the tag and still writes an empty report.

```sh
bazel run @rules_dx//:dx -- coverage //...
bazel run @rules_dx//:dx -- coverage --min-coverage 100 //...
```

Coverage ignores use `LCOV_EXCL_LINE` or `LCOV_EXCL_START` / `LCOV_EXCL_STOP`
with a short `reason:` and `issue:`. Bare ignores without a reason fail the
gate. `reason:` goes on the marker line or the line before and holds at most 120
characters. `issue:` must contain a digit.

The comment syntax depends on the language: `//` for Rust, Go, JS/TS, the JVM
languages (including Kotlin scripts, `.kts`), .NET and C/C++; `#` for Python
and Ruby; `%` for Erlang (`.erl` and `.hrl`). Markers in other sources
(`.bzl`, `.pyi`, `.cu`/`.cuh`, `.d.ts`) are not read.

## Line And Branch Metrics

This section describes `//tools/coverage:coverage_bin`, not `dx coverage`.

The coverage gate enforces exact executable-line coverage. Branch outcomes
are measured separately and never change the gate. Each file with branch
records reports raw, ignored, eligible and taken counts with its language.
Branches on lines covered by `LCOV_EXCL_LINE` or a START/STOP range are
counted as ignored and are left out of eligible, taken, missed and the rate.
The declared branch summary is still checked against all records. Files without
branch records report unavailable instead of a rate. A declared branch
summary that does not match its records fails the report as incomplete.
