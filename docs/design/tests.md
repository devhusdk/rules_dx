# Tests design

Every test target and test macro follows these rules.

## Default set

- Every test runs in the default set: `bazel test` on its package pattern selects it. A conformance subject is the one exception.
- No test carries the `manual` tag.
- A test that cannot run there is fixed or deleted.
- A test for only some platforms says so with `target_compatible_with`, not a tag.

## Sandbox, remote execution and cache

- Every test runs sandboxed, can run on a remote executor, and has its result cached.
- No test carries `local`, `no-sandbox`, `no-cache`, `no-remote`, `external` or `requires-network`.
- No test sets `local = True`.
- A test reads only its declared inputs: never the checkout, the home directory or the network.
- A test writes only under `TEST_TMPDIR` and its declared outputs.

## Host tools

- A test runs no host tool.
- Every program a test starts is a runfile or comes from a pinned toolchain checked by `sha256`.
- A test starts a program by its runfiles path with an empty `PATH`.
- The rules in `docs/design/host-tools.md` apply to tests.

## Exclusive

- `exclusive` is not allowed.
- Tests that share a resource each get their own copy of it.
- A resource that Bazel can count is declared with a `resources:<name>:<n>` tag.

## Test kinds

A test is the first of four kinds that can prove the behaviour.

| Kind | What it is | Allowed when |
| --- | --- | --- |
| Unit | A `rust_test`, or a Starlark unit test over pure functions. | The behaviour is a function of its arguments or of declared files. |
| Analysis | A Starlark analysis test over providers, actions and failures. | The behaviour is what a rule declares: providers, action arguments, inputs, outputs or an analysis error. |
| In-graph fixture | A test that depends on real fixture targets in this workspace and checks their built outputs. | The behaviour needs an action to run: a compiler, a linter, a packager. |
| Nested | A test that runs a second Bazel in a consumer workspace. | No other kind can prove the same thing. |

- A test that only checks rule behaviour is an analysis test or an in-graph fixture test.
- A fixture target that must fail to build is the subject of an analysis failure test, not a test target.
- A fixture is a library, binary or file target, or a conformance subject.

## Nested Bazel

- Every test that needs Bazel uses the one shared harness in `tools/testing/nested`.
- The harness runs the pinned Bazel from runfiles.
- The nested build is offline.
- Every repository the nested build reads is a declared input of the test.
- The nested build sees no host `PATH`.
- Each nested test is listed with its measured time.

## Speed

- The target is under 1 second per test.
- A unit, analysis or in-graph fixture test starts no Bazel server.
- A change that adds a nested test states its measured time.

## Expected failures

- A target that must fail is a `testonly` non-test executable such as `starlark_binary`.
- A passing test runs that executable from runfiles and asserts its exit code and output.
- An analysis failure is asserted with `failure_test` on the non-test target.
- A language test that must fail is a conformance subject.
- A compile that must fail is recorded by `compile_log` in `//tools/testing/hermetic:compilers.bzl`. An action replays the compile action of the fixture library and writes the exit code and the output to a file. A passing test asserts both.

## Conformance

- Every directory with a `lang.json` calls `dx_conformance_suite` from `//tools/testing/conformance:defs.bzl` once, in `<lang>/tests/conformance/BUILD.bazel`.
- The package declares fixtures: files and targets of the `lang.json` `entry_points`. The suite declares every test and holds every assertion.

| Test | Passes when |
| --- | --- |
| `build_test` | The fixture binary prints the expected line and exits 0. |
| `pass_test` | The passing subject test exits 0. |
| `fail_test` | The failing subject test exits non-zero and names its failing case. |
| `coverage_test` | The passing subject test, built with `collect_code_coverage`, writes LCOV with a hit on the fixture library. |
| `lint_test` | The lint aspect reports nothing on the clean fixture. |
| `lint_error_test` | The lint aspect reports the expected rule, file and line on the dirty fixture. |
| `format_test` | The format aspect reports nothing on the clean fixture. |
| `format_error_test` | The format aspect reports the unformatted fixture file. |
| `lock_test` | The fixture binary imports a package pinned by the checked-in lockfile. The lock check rejects a lockfile without it. |
| `cross_test` | The fixture binary, built for another platform of the language, has that platform's object format, OS and CPU. |
| `runfiles_test` | The fixture binary reads a `data` file through the language's runfiles library and prints it. |

- A subject test is a fixture declared with the suite's `subject_rule`: a `lang.json` `entry_points` name that declares a test target. Outside the suite's transition it is incompatible, so a pattern skips it.
- A language without one lists `pass_test`, `fail_test` and `coverage_test` in `not_applicable`.
- A suite test is an in-graph fixture test with `size = "small"`. It runs a subject from runfiles with Bazel's test environment.
- `lang.json` `platforms` is never empty. A suite test runs on each of them and is incompatible on any other.
- The suite's `not_applicable` maps a test, or a test and platform, to a reason. Nothing else omits a suite test.
- `tools/testing/conformance/pending.txt` holds a `<lang> <test> <platform>` line per suite test that does not pass yet. Such a test is incompatible on that platform. The list only shrinks.
- The suite declares `:manifest`, compatible on every platform, with no dependency on a suite test. It records the label, the reason or `pending` of each test on each platform.
- `//tools/testing/conformance:manifest` depends on the `:manifest` of every language in `core/modules/langs.bzl`. A language without a suite fails its build.
- `docs/support-matrix.md` takes each suite test cell from the manifest. No hand-written table states one.

## Wrapper upstream

- A public test wrapper is the test. Its private `<name>_upstream` target is the executable the wrapper runs.
- `DX_UPSTREAM_TAG` in `core/wrapper.bzl` is `manual`. `dx_upstream_tags` is its only writer.
- Every private upstream target carries `DX_UPSTREAM_TAG`. Nothing else does.
- No BUILD file tags an upstream target.
- Expected-tag data names the upstream tag as `DX_UPSTREAM_TAG`, never as a string.
- A tag-forwarding probe uses a tag that is not banned.
- A test whose tags a probe reads runs in the default set.

## Duplicates

- A test that proves what another test already proves is deleted.
- A test that cannot fail is deleted.

## Guard

- `@dx_test_tags_sources//:sources.json` snapshots every `BUILD`, `BUILD.bazel` and `.bzl` file. Its repository rule watches every path it reads.
- `@dx_test_tags_sources//:files.json` snapshots `MODULE.bazel`, `<lang>/lang.json`, `<lang>/README.md`, repository `*.BUILD` and `*.BUILD.bazel` files and the `docs/` language and command pages. `@dx_test_tags_sources//:paths.json` lists every path under language directories.
- A test that walks the module tree declares the snapshots, sets `DX_SOURCES_JSON`, `DX_FILES_JSON` and `DX_PATHS_JSON`, and reads them with `dx_testing::source_snapshot()`. A Python test writes them into a scratch tree.
- `//tools/ci:test_tags_test` scans that snapshot as a declared input.
- It counts `manual`, `exclusive` and the tags banned under Sandbox, remote execution and cache.
- It counts a tag in the `tags` of a test rule or test macro call, in `local = True` on such a call, and in a tags value built outside a call: a tags-named variable, a `"tags"` dict entry, or the arguments of a tags-named function.
- It counts a loaded tags constant at each test call that uses it.
- It counts `DX_UPSTREAM_TAG` in the `tags` of a test call as `manual`.
- A call is a test call when its name has the word `test`, `tests`, `suite`, `bench` or `fuzz`, when the name is a `rule(test = True)`, or when the name is a macro that calls one of these.
- It skips `tags` on a call that is not a test call.
- `tools/ci/test_tags_allowlist.txt` holds one line per file and tag: `<path> <tag> <count>`. Lines are sorted.
- The guard fails when a file has more occurrences than its line allows.
- The guard fails when a line allows more than the file has, or names a file with none.
- The allowlist only shrinks. No change adds a line or raises a count.
- `@dx_test_tags_sources//:code.json` snapshots every `.rs`, `.sh` and `.tpl` file, `.bazelrc` and `.dx/bazelrc/*.bazelrc`.
- `//tools/ci:sandbox_escape_test`, `//tools/ci:host_env_test` and `//tools/ci:host_program_test` scan `sources.json`, `code.json` and the repository `*.BUILD` files of `files.json`. They read `DX_SOURCES_JSON`, `DX_FILES_JSON` and `DX_CODE_JSON` as maps and write no scratch tree.
- `//tools/ci:sandbox_escape_test` has three kinds:
  - `exec-requirement`: `requires-network`, `no-sandbox`, `no-remote`, `no-cache` or `local` as a key of execution requirements, in the `tags` of a call that is not a test call, or added by `--modify_execution_info`. `local` counts as a key only in an `execution_requirements` argument or a requirements-named variable.
  - `tmp-path`: a string or shell word that is `/tmp` or starts with `/tmp/`, and `env::temp_dir()` in Rust outside test files and `#[cfg(test)]` items.
  - `rc-flag`: `--incompatible_strict_action_env` or `--sandbox_default_allow_network=false` set by no line without a config in `.bazelrc` and `.dx/bazelrc/*.bazelrc`. The line is `.bazelrc rc-flag <missing flags>`.
- `docs/design/host-tools.md` section Checks lists the kinds of the other two.
- Each has its own allowlist, `tools/ci/<name>_allowlist.txt`, with lines `<path> <kind> <count>` under the same rules as the tags allowlist.
- The three guards skip their own sources.
