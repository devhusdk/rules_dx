# Build, test, and coverage

Run Bazel over the scope and report the result.

```sh
dx build //...
dx test --affected
dx test //pkg/... -- --test_filter=case_name
dx coverage --min-coverage 80 --report lcov=lcov.info //pkg/...
dx run //app:bin -- --port=8080
dx deploy --apply //app:deploy
```

## `dx build`

```text
dx build [--here] [--affected] [--debug|--release] [--explain] [--profile] [--report failures=<path>] [scope...] [-- bazel-options...]
```

## `dx test`

```text
dx test [--here] [--affected] [--debug|--release] [--explain] [--profile] [--report junit|failures|results=<path>] [scope...] [-- bazel-options...]
```

## `dx coverage`

```text
dx coverage [--here] [--affected] [--explain] [--profile] [--min-coverage <percent>] [--min-changed-line-coverage <percent>] [--report lcov|sources|failures|results=<path>] [scope...] [-- bazel-options...]
```

## Flags

| Flag | What it does |
| --- | --- |
| `--here` | Use the current directory tree. |
| `--affected` | Use the targets your git changes reach. |
| `--debug` | Compile with the `dx_debug` profile (`build`, `test`, `run`, `deploy`). |
| `--release` | Compile with the `dx_release` profile. No flag uses `dx_dev`. |
| `--explain` | Save the execution log for [`dx explain`](explain.md). |
| `--profile` | (`build`, `test`, `coverage`, `run`; `run` reports on its build step.) Pass `--profile` to Bazel and print where build time went: wall time, critical path with its top five actions (with Bazel mnemonics when the trace has them), cache hit rate (disk, remote, local), bytes downloaded, Bazel peak heap and CPU time per action mnemonic (from the build event stream, absent for `run`), the ten slowest actions and repository fetch time. The trace is `.dx/profile/<run id>.json.gz` (the last five are kept) and the report `.dx/profile/report.json`; with `--output=json` the report is the message of a `profile_report` notice. |
| `--min-coverage <percent>` | `coverage`: fail below this whole percent (0-100). |
| `--min-changed-line-coverage <percent>` | `coverage`: fail below this whole percent (0-100) on lines changed since the merge-base. |
| `--report <format>=<path>` | Write a report. Repeatable. `-` is stdout. |
| `-o, --output text\|json` | Choose the output format. |
| `--workspace`, `-q`, `-v`, `--color`, `--build-slots`, `--bazel-startup-option` | Global flags. |

`--debug` and `--release` conflict. Both change compilation only.

## Scopes

No scope means `//...`. Pass labels, patterns, files, or directories. A file runs its owning targets. For `test` and `coverage`, a file runs the tests that depend on its owners. A `manual` test runs only when it owns the file.

`--here` and `--affected` do not combine with scopes or each other. See [Scope defaults](scope-defaults.md).

`--affected` on `test` and `coverage` runs only affected tests. With nothing affected, dx prints `no affected targets` and exits `0`.

Args after `--` go to Bazel. Bazel startup options after `--` exit `2`.

## Protected flags

dx sets these flags. Another value exits `2` before Bazel starts.

- `--@rules_dx//config:clippy_output_diagnostics=true`
- `--@rules_dx//config:rustc_output_diagnostics=true`
- `--skip_incompatible_explicit_targets`
- `--remote_download_outputs=toplevel` for `test` and `coverage`. Pass `--remote_download_outputs=all` or `--remote_download_all` to download everything.
- `--combined_report=lcov` for `coverage`.
- `--build_event_json_file`.

`coverage` also refuses `--build_tests_only`.

## Output

Text ends with `dx: <total> actions, <executed> executed, <cached> cached` on stderr. `--quiet` hides it.

Each Bazel failure prints a `dx: build failure:` line with its rustc diagnostics. JSON adds a `build_failure` event per failure.

`dx test` prints `dx: failed: //pkg:test: case_a, case_b and 2 more (log: <path>/test.log)` per failed result. JSON adds a `test_outcome` event per result that did not plainly pass.

An agent session defaults to `--output=json`. `--output` and `DX_OUTPUT` win.

## Reports

Relative paths anchor at the workspace root. Parent directories must exist. Two reports with the same path fail before anything runs.

| Report | Command | Content |
| --- | --- | --- |
| `junit` | `test` | JUnit XML. |
| `results` | `test`, `coverage` | JSON with every test result. |
| `failures` | `build`, `test`, `coverage` | JSON with this run's failures. Written on success with an empty list. |
| `lcov` | `coverage` | Combined LCOV. |
| `sources` | `coverage` | JSON with the owned, absent, and orphan sources. |

With `--output=json` and no `results` report, `dx test` writes `.dx/results.json`.

The `results` file has `record_version`, `exit_code`, `run_id`, `complete`, and `results`. `run_id` matches `command_finished.run_id`.

The `failures` file has `record_version`, `exit_code`, `complete`, `truncated`, `failures_omitted`, and `failures`. Each failure has `target`, `configuration`, `stage`, `category`, `action`, `diagnostics`, and a `stderr` object. dx drops command lines and never reports environment variables.

## Test details

- Args after `--` reach the test binary through Bazel. `--test_arg` works on `test` and `coverage` only.
- `--test_filter` stays a Bazel option. For `rust_test` targets that use libtest, dx also passes it to the binary.
- Tests run once. Only tests marked flaky retry, up to three attempts. A pass after retry reports `passed_after_retry`.
- To retry one test: `dx test <label> -- --flaky_test_attempts=3`.
- A test Bazel skipped has outcome `skipped`. A test a tag or size filter removed has outcome `filtered`, and dx prints `tests_filtered`.

## Coverage details

`--min-coverage` counts the eligible lines of every production source in the scope, including the production sources its targets depend on, such as a shared library outside the patterns. It also fails on a source missing from the LCOV, an LCOV path no target owns, or an invalid exclusion marker. A passing gate prints `coverage <percent>% (<covered>/<eligible> lines) meets minimum <n>% in cell <cell>`. A failing gate exits `1` with `coverage_below_minimum`.

`--min-changed-line-coverage` gates the lines added since the merge-base with the default branch (`origin/HEAD`, else `main`, else `master`). It needs full Git history. It exits `1` below the minimum, for a changed source the LCOV never saw, or on a Git failure.

A whole-repository run (`//...`) also fails on a tracked source no Bazel target owns. Add the file to a rule or delete it.

The correctness preset sets `coverage --instrument_test_targets`, so sources a test compiles itself are counted. Test-only sources count as support, not as eligible lines.

A test tagged `dx-no-coverage-collector` fails with `no_coverage_collector`. Tag a test `no-coverage` to skip it, or add `coverage --test_tag_filters=-no-coverage,-dx-no-coverage-collector` to `.bazelrc`.

Zig and Fortran tests run under kcov, which runs on Linux only. On macOS and Windows a Zig or Fortran test writes no coverage. The LCOV counts the sources of the libraries in `deps`, plus the test sources (the preset instruments test targets).

Exclude code with `LCOV_EXCL_LINE` or `LCOV_EXCL_START` / `LCOV_EXCL_STOP`. Each marker needs `reason: <text>` (up to 120 characters) on the marker line or the line before, and `issue: <text>` containing a digit. Use `//` in Rust, Go, JS/TS, JVM languages, .NET, D, PHP, Zig, and C/C++. Use `#` in Python, Ruby, Perl, PowerShell, Julia, and Elixir. Use `%` in Erlang. Use `--` in Haskell. Use `(* *)` in OCaml. A marker that covers no instrumented line, or that excludes every instrumented line of a file, fails `--min-coverage`.

## `dx run`

```text
dx run [--debug|--release] [--profile] [--bazel-option <token>]... <label...> [-- args...]
```

Builds the runnable targets, then runs them in order. Args after `--` go to the app. `--bazel-option <token>` passes one option to the build and the run. Repeatable.

- Labels and patterns run in order. A file or directory scope must match exactly one runnable target.
- The first app that exits nonzero stops the rest.
- `--apply` is a usage error. Works with `CI=true`.
- Output: `--output text|json`. No reports. Text prints `dx: <label> exited <code>`.

## `dx deploy`

```text
dx deploy [--apply] [--debug|--release] <label> [-- args...]
```

Builds one deployable target. `--apply` runs the deployment and sets `DX_PROFILE`. Without a flag the profile comes from the target's `profile` attribute. Args after `--` reach the app on `--apply` only. Output: text only.

### Registries released by tag

Go, Julia, Swift, PHP, opam and CPAN have no `dx deploy` target. Release them by hand:

- Go, Swift: push a tag (`vX.Y.Z` for Go, a semver tag for Swift).
- PHP: run `composer validate`, then push a tag.
- Julia: push a tag, then comment `@JuliaRegistrator register` on the commit.
- opam: open a pull request to the opam repository.
- CPAN: upload the tarball to PAUSE.

## Signals

Ctrl-C, `SIGTERM`, and `SIGHUP` stop the run. Children get the signal, and any still running after 10 seconds are killed. dx dies by the same signal, so the shell sees `128` plus the signal number.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | Operational failure, `no_tests`, incomplete test results, incomplete coverage, coverage below minimum, a failed report write, or an app launch failure. |
| `2` | Usage, scope, or owner error. |
| Bazel's code | Bazel failed. `3` means tests failed. |
| `128`+N | The app or dx died by signal N. |

Incomplete results fail the run even when Bazel passes. Examples are a missing `test.xml` or build events that end early.
