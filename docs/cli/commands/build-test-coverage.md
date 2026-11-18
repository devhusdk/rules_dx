# Build, test, and coverage

Use `dx build`, `dx test`, `dx coverage` and `dx run` to build your code, run its tests, measure test coverage, and start a program.

```sh
dx build //...
dx test --affected
dx test //pkg/... -- --test_filter=case_name
dx coverage --min-coverage 80 --report lcov=lcov.info //pkg/...
dx run //app:bin -- --port=8080
```

A few Bazel words this page uses:

- A *target* is one thing Bazel can build or run, such as a library, a binary or a test.
- A *package* is a directory with a `BUILD.bazel` file. It holds targets.
- A *label* names one target. `//pkg:name` is the target `name` in the package `pkg`.
- A *pattern* names many targets. `//pkg/...` is every target in `pkg` and below it. `//...` is every target in the workspace.

## `dx build`

Build targets.

```text
dx build [--here] [--affected [--since <ref>|--base <ref>]] [--debug|--release] [--timings] [--no-retry] [--report failures=<path>] [scope...] [-- bazel-options...]
```

```sh
dx build //...
dx build --release //app -- --local_resources=cpu=4
```

## `dx test`

Build and run tests.

```text
dx test [--here] [--affected [--since <ref>|--base <ref>]] [--debug|--release] [--timings] [--no-retry] [--report junit|failures|results=<path>] [scope...] [-- bazel-options...]
```

```sh
dx test --affected
dx test //pkg/...
dx test //pkg:test -- --test_filter=case_name
```

## `dx coverage`

Run tests and collect LCOV coverage. Optionally fail below a minimum.

```text
dx coverage [--here] [--affected [--since <ref>|--base <ref>]] [--timings] [--no-retry] [--fail-on info|warning|error] [--min-coverage <percent>] [--min-changed-line-coverage <percent>] [--report lcov|sources|failures|results=<path>] [scope...] [-- bazel-options...]
```

```sh
dx coverage //pkg/...
dx coverage --min-coverage 80 --report lcov=lcov.info
```

## `dx run`

Build runnable targets, then run them one after another.

```text
dx run [--debug|--release] [--timings] [--bazel-option <token>]... <label...> [-- app-args...]
```

```sh
dx run //app
dx run --release //app -- --port 8080
```

- Args after `--` go to the app, not to Bazel.
- `--bazel-option <token>` passes one Bazel option to the build and the run. Repeat it for more.
- With no scope, dx runs the only executable target under `//...`. If there are several, it fails with `pass one of: ...` and lists up to 10.
- Labels and patterns run in the order you give. A file or directory scope must match exactly one runnable target.
- The first app that exits nonzero stops the rest.
- `dx run` never publishes. Use `dx deploy` for that. An apply flag is a usage error. `dx run` works with `CI=true`.
- Text output prints `dx: <label> exited <code>` for each app. `dx run` writes no reports.

## Flags

| Flag | Commands | What it does |
| --- | --- | --- |
| `--here` | `build`, `test`, `coverage` | Use the current directory tree: `//path/...`, or `//...` at the workspace root. |
| `--affected` | `build`, `test`, `coverage` | Use only the targets your git changes reach. |
| `--since <ref>` | `build`, `test`, `coverage` | With `--affected`: compare against this git ref instead of the merge-base with the default branch. Without `--affected`, exits `2`. |
| `--base <ref>` | `build`, `test`, `coverage` | With `--affected`: take the merge-base with this branch instead of the default branch. Not with `--since`. |
| `--debug` | `build`, `test`, `run` | Compile with the `dx_build_debug` build mode. |
| `--release` | `build`, `test`, `run` | Compile with the `dx_build_release` build mode. |
| `--timings` | all four | Print where build time went. For `run`, it reports on the build step. |
| `--no-retry` | `build`, `test`, `coverage` | Do not run Bazel again without the remote cache after a remote cache error. |
| `--fail-on info\|warning\|error` | `coverage` | Fail on findings of this severity or worse. Default `warning`. |
| `--min-coverage <percent>` | `coverage` | Fail below this whole percent (0-100). |
| `--min-changed-line-coverage <percent>` | `coverage` | Fail below this whole percent (0-100) on lines changed since the merge-base. |
| `--bazel-option <token>` | `run` | Pass one Bazel option to the build and the run. Repeatable. |
| `--report <kind>=<path>` | `build`, `test`, `coverage` | Write a report. Repeatable. `-` is stdout. |
| `-o, --output text\|json` | all four | Choose the output format. |
| `--workspace <dir>` | all four | Use this workspace directory. |
| `-q, --quiet` | all four | Hide summaries. |
| `-v, --verbose` | all four | Show more logs, including full Bazel output. |
| `--color auto\|always\|never` | all four | Set color output. |
| `--build-slots <auto\|n\|off>` | all four | Set build slots: `auto`, a count, or `off`. |
| `--build-slots-wait <duration>` | all four | Give up waiting for a build slot after this long. `0` fails at once. |
| `--bazel-startup-option <token>` | all four | Pass one Bazel startup option. Repeatable. |

With neither `--debug` nor `--release`, dx uses the `dx_build_dev` build mode. `--debug` and `--release` conflict. Both change compilation only.

`--timings` passes `--profile` to Bazel and prints:

- wall time;
- the critical path and its top five actions, with Bazel mnemonics when the trace has them;
- the cache hit rate (disk, remote, local) and bytes downloaded;
- Bazel peak heap;
- CPU time per action mnemonic, from the build event stream (absent for `run`);
- the ten slowest actions and the repository fetch time.

The trace goes to `.dx/local/timings/<run id>.json.gz`. dx keeps the last five. The report goes to `.dx/local/timings/report.json`. With `--output=json`, the report is the message of a `timings_report` notice.

When a Bazel `build`, `test` or `coverage` call fails and Bazel's own output (not test output) has an `ERROR:` line about the remote cache, dx runs it once more with `--remote_cache= --noremote_accept_cached`. dx prints a notice when it does, and the summary on stderr says how many calls ran again. `--no-retry` turns this off.

## Scopes

A scope picks what to run. Pass labels, patterns, files, or directories. With no scope, dx uses `//...`.

- A file runs the targets that own it.
- For `test` and `coverage`, a file runs the tests that depend on its owning targets.
- A test tagged `manual` runs only when it owns the file. A *tag* is a word in a target's `tags` list.
- `--here` uses the current directory tree.
- `--affected` uses the targets changed since the merge-base with the default branch (or with `--base <ref>`), or since `--since <ref>`, plus the targets that depend on them. On `test` and `coverage` it runs only the affected tests. With nothing affected, dx prints `no affected targets` and exits `0`.
- `--here` does not combine with scopes or `--affected`.
- `--affected` with scopes keeps only the affected targets inside them, for example `dx test --affected //svc/...`.

`dx test //...` in a workspace with no test targets prints `the workspace has no test targets` and exits `0`. A narrower pattern with no tests exits `1` with `no_tests`.

For more on scopes, read [Scope defaults](scope-defaults.md).

## Bazel options

Args after `--` go to Bazel, except on `dx run`. Bazel startup options after `--` exit `2`. Use `--bazel-startup-option` for those.

```sh
dx build //app -- --local_resources=cpu=4
```

When stdout is not a terminal, dx passes Bazel these quiet flags: `--noshow_progress`, `--noshow_loading_progress`, `--noshow_timestamps`, `--curses=no`, `--ui_event_filters=-info,-debug` and `--test_summary=none`. `-v` keeps full Bazel output.

`build`, `test` and `coverage` also pass `--skip_incompatible_explicit_targets`, so targets that cannot build on this platform are skipped.

## Protected flags

dx sets these Bazel flags. If you pass another value after `--`, dx exits `2` before Bazel starts.

- `--@rules_dx//config:clippy_output_diagnostics=true` and `--@rules_dx//config:rustc_output_diagnostics=true`, plus the same settings under `@rules_rust//rust/settings`.
- `--build_event_json_file` and `--build_event_json_file_path_conversion`.
- `--remote_download_outputs=toplevel` for `test` and `coverage`. Pass `--remote_download_outputs=all` or `--remote_download_all` to download everything. `--remote_download_minimal` is refused.
- `--combined_report=lcov` for `coverage`.
- `--build_tests_only` for `coverage`. Only `--build_tests_only=false` is accepted.

## Output

Text output ends with `dx: <total> actions, <executed> executed, <cached> cached` on stderr. `--quiet` hides it.

Each Bazel failure prints a `dx: build failure:` line with its compiler diagnostics. dx reads rustc, clippy, and the gcc-style output of cc, javac, go, kotlinc, scalac, tsc, csc and swiftc. JSON adds a `build_failure` event per failure.

`dx test` prints one line per failed result:

```text
dx: failed: //pkg:test: case_a, case_b and 2 more (log: <path>/test.log)
```

JSON adds a `test_outcome` event per result that did not plainly pass. Each case in it has `classname`, `name`, `outcome` and `message`. It also has `file` and `line` when the report's `file` and `line` attributes, or the first workspace-relative `path:line` in the failure text, give them. `results.json` cases carry the same fields.

`dx coverage` prints the total, such as `dx: coverage 83.4% (1204/1444 lines)`. With `--min-coverage`, the minimum check prints the total instead. Then it lists up to ten least covered files as `percent covered/eligible path`. JSON carries the totals in the `coverage` event.

`--output json` streams events while the command runs. An agent session defaults to `--output=json`. `--output` and `DX_OUTPUT` win.

## Reports

Pass `--report <kind>=<path>`. Repeat it for more reports. `-` writes to stdout.

| Report | Commands | Content |
| --- | --- | --- |
| `junit` | `test` | JUnit XML. |
| `results` | `test`, `coverage` | JSON with every test result. |
| `failures` | `build`, `test`, `coverage` | JSON with this run's failures. Written on success with an empty list. |
| `lcov` | `coverage` | Combined LCOV. |
| `sources` | `coverage` | JSON with the owned, absent, and orphan sources. |

- Relative paths start at the workspace root.
- Parent directories must exist.
- Two reports with the same path fail before anything runs.
- With `--output=json` and no `results` report, `dx test` writes `.dx/local/results.json`.

The `results` file has `record_version`, `exit_code`, `run_id`, `complete`, and `results`. `run_id` matches `command_finished.run_id`.

The `failures` file has `record_version`, `exit_code`, `complete`, `truncated`, `failures_omitted`, and `failures`. Each failure has `target`, `configuration`, `stage`, `category`, `action`, `diagnostics`, and a `stderr` object. dx drops command lines and never reports environment variables.

## Keeping `.dx/local` in step

After `dx build`, `dx test`, `dx coverage`, `dx run` or `dx check` succeeds in a workspace that `dx sync` has set up, dx checks whether any of these changed since the last sync:

- the generated files the selected setup links to;
- `MODULE.bazel`, `MODULE.bazel.lock`, `.bazelversion`, `.bazelrc`, `dx.toml` or `.dx/pins`.

This check makes no Bazel call. When something changed, dx runs `dx sync` with the same scope and prints `dx: synced .dx/local (codegen changed)` on stderr.

A failed sync prints one `dx: warning: sync_failed: <reason>; next: dx sync` and keeps the exit code. Later runs repeat the warning without calling Bazel until the inputs change.

CI, `--quiet` and `--here` skip this check. It cannot see a new codegen target, or a generated file the command did not build. Run `dx sync` for those.

## Test details

- Args after `--` reach the test binary through Bazel. `--test_arg` works on `test` and `coverage` only.
- `--test_filter` stays a Bazel option. For `rust_test` targets that use libtest, dx also passes it to the test binary.
- Tests run once. Only tests marked flaky retry, up to three attempts. A pass after a retry reports `passed_after_retry`.
- To retry one test: `dx test <label> -- --flaky_test_attempts=3`.
- A test Bazel skipped has outcome `skipped`.
- A test a tag or size filter removed has outcome `filtered`, and dx prints `tests_filtered`.
- Incomplete results fail the run even when Bazel passes, for example a missing `test.xml` or build events that end early.

## Coverage details

`--min-coverage` counts the eligible lines of every production source in the scope. That includes the production sources the targets depend on, such as a shared library outside your patterns. It also fails on:

- a source missing from the LCOV;
- an LCOV path no target owns;
- an invalid exclusion marker.

A passing gate prints `coverage <percent>% (<covered>/<eligible> lines) meets minimum <n>% in cell <cell>`. A failing gate exits `1` with `coverage_below_minimum`.

`--min-changed-line-coverage` checks the lines added since the merge-base with the default branch: `origin/HEAD`, else `main`, else `master`. It needs full Git history. It exits `1` below the minimum, for a changed source the LCOV never saw, or on a Git failure.

A whole-workspace run (`//...`) also fails on a tracked source that no Bazel target owns. Add the file to a rule or delete it.

The correctness preset sets `coverage --instrument_test_targets`, so sources a test compiles itself are counted. Test-only sources count as support, not as eligible lines.

A test tagged `dx-no-coverage-collector` is a warning finding. At the default `--fail-on=warning`, `dx coverage` fails with `no_coverage_collector`. With `--fail-on=error`, it skips the test and prints one summary line per language. To skip a test on purpose, tag it `no-coverage`, or add this line to `.bazelrc`:

```text
coverage --test_tag_filters=-no-coverage,-dx-no-coverage-collector
```

Zig and Fortran tests run under kcov, which runs on Linux only. On macOS and Windows a Zig or Fortran test writes no coverage. The LCOV counts the sources of the libraries in `deps`, plus the test sources.

On Windows, C and C++ sources count as unsupported files, because no C++ coverage collector runs there.

## Exclude code from coverage

Mark one line with `LCOV_EXCL_LINE`. Mark a block with `LCOV_EXCL_START` and `LCOV_EXCL_STOP`.

Each marker needs both:

- `reason: <text>`, up to 120 characters, on the marker line or the line before;
- `issue: <text>`, where the text contains a digit.

```text
let value = parse(input)?; // LCOV_EXCL_LINE - reason: parse never fails on fixed input, issue: 42
```

Write the marker in a comment of the file's language:

| Comment | Languages |
| --- | --- |
| `//` | Rust, Go, JS/TS, JVM languages, .NET, D, PHP, Zig, C and C++ |
| `#` | Python, Ruby, Perl, PowerShell, Julia, Elixir |
| `%` | Erlang |
| `--` | Haskell, Lua |
| `(* *)` | OCaml |

A marker that covers no instrumented line fails `--min-coverage`. So does a marker that excludes every instrumented line of a file.

## Signals

Ctrl-C, `SIGTERM`, and `SIGHUP` stop the run. Child processes get the signal. Any still running after 10 seconds are killed. dx then dies by the same signal, so the shell sees `128` plus the signal number.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | Operational failure, `no_tests`, incomplete test results, incomplete coverage, coverage below minimum, a failed report write, or an app that failed to launch. |
| `2` | Usage, scope, or owner error. |
| Bazel's code | Bazel failed. dx keeps Bazel's exit code. `3` means tests failed. |
| `128`+N | The app or dx died by signal N. |
