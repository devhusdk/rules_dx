# Tests design

This page fixes how tests in this repository are declared and run. Every test target and test macro follows it.

## Default set

- Every test runs in the default set: `bazel test` on its package pattern selects it.
- No test carries the `manual` tag.
- A test that cannot run in the default set is fixed or deleted.
- A test that runs only on some platforms says so with `target_compatible_with`, not with a tag.

## Sandbox, remote execution and cache

- Every test runs sandboxed, can run on a remote executor, and has its result cached.
- No test carries `local`, `no-sandbox`, `no-cache`, `no-remote`, `external` or `requires-network`.
- No test sets `local = True`.
- A test reads only its declared inputs. It never reads the checkout, the home directory or the network.
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

A test is one of four kinds. Pick the first kind that can prove the behaviour.

| Kind | What it is | Allowed when |
| --- | --- | --- |
| Unit | A `rust_test`, or a Starlark unit test over pure functions. | The behaviour is a function of its arguments or of declared files. |
| Analysis | A Starlark analysis test over providers, actions and failures. | The behaviour is what a rule declares: providers, action arguments, inputs, outputs or an analysis error. |
| In-graph fixture | A test that depends on real fixture targets in this workspace and checks their built outputs. | The behaviour needs an action to run: a compiler, a linter, a packager. |
| Nested | A test that runs a second Bazel in a consumer workspace. | No unit, analysis or in-graph test can prove the same thing. |

- A test that only checks rule behaviour is an analysis test or an in-graph fixture test.
- A fixture target that must fail to build is the subject of an analysis failure test. It is not a test target.
- A fixture is a library, binary or file target. A test target is the fixture of another test only as a conformance subject.

## Nested Bazel

- Every test that needs Bazel uses the one shared harness in `tools/testing/nested`.
- The harness runs the pinned Bazel from runfiles.
- The nested build is offline. It downloads nothing.
- Every repository the nested build reads is a declared input of the test.
- The nested build sees no host `PATH`.
- The result is cached like any other test.
- No test starts Bazel any other way.
- Each nested test is listed with its measured time.

## Speed

- The target is under 1 second per test, with no server startup.
- A unit, analysis or in-graph fixture test starts no Bazel server.
- A change that adds a nested test states its measured time.

## Expected failures

- A target that must fail is a `testonly` non-test executable such as `starlark_binary`.
- A passing test runs that executable from runfiles and asserts its exit code and output.
- An analysis failure is asserted with `failure_test` on the non-test target.
- A language test that must fail is a conformance subject.

## Conformance

- Every directory with a `lang.json` calls `dx_conformance_suite` from `//tools/testing/conformance:defs.bzl` once, in `<lang>/tests/conformance/BUILD.bazel`.
- The package declares fixtures with the macros of `<lang>/defs.bzl`. The suite declares every test and holds every assertion.

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

- A subject test is a `<lang>_test` fixture that is incompatible outside the suite's transition. A package pattern skips it.
- A suite test runs a subject from runfiles with Bazel's test environment.
- A suite test is an in-graph fixture test with `size = "small"`.
- A suite test runs on every platform in `lang.json` `platforms`.
- `not_applicable` maps a capability, or a capability and platform, to a reason. Nothing else omits a suite test.
- `tools/testing/conformance/pending.txt` holds one line per suite test that does not pass yet: `<lang> <test> <platform>`. Such a test is incompatible on that platform. The list only shrinks.
- `//tools/testing/conformance:manifest` depends on every suite test of every language in `core/modules/langs.bzl`. A missing test fails its build.
- `docs/support-matrix.md` takes each capability cell from the manifest: the test label, the reason or `pending`. No hand-written table states a capability.

## Duplicates

- A test that proves what another test already proves is deleted.
- A test that cannot fail is deleted.

## Guard

- `@dx_test_tags_sources//:sources.json` snapshots every `BUILD`, `BUILD.bazel` and `.bzl` file of the module. Its repository rule watches each directory and file it reads.
- `@dx_test_tags_sources//:files.json` snapshots the `MODULE.bazel`, `<lang>/lang.json`, repository `*.BUILD` and `*.BUILD.bazel` files and the language and command pages under `docs/`.
- A test that walks the module tree declares both snapshots, passes them through `DX_SOURCES_JSON` and `DX_FILES_JSON`, and reads them with `dx_testing::source_snapshot()`. A Python test writes them into a scratch tree.
- `//tools/ci:test_tags_test` scans that snapshot as a declared input.
- It counts `manual`, `exclusive` and the tags banned under Sandbox, remote execution and cache.
- It counts a tag in the `tags` of a test rule or test macro call, in `local = True` on such a call, and in a tags value built outside a call: a tags-named variable, a `"tags"` dict entry, or the arguments of a tags-named function.
- It counts a loaded tags constant at each test call that uses it.
- A call is a test call when its name has the word `test`, `tests`, `suite`, `bench` or `fuzz`, when the name is a `rule(test = True)`, or when the name is a macro that calls one of these.
- It skips `tags` on a call that is not a test call.
- `tools/ci/test_tags_allowlist.txt` holds one line per file and tag: `<path> <tag> <count>`. Lines are sorted.
- The guard fails when a file has more occurrences than its line allows.
- The guard fails when a line allows more than the file has, or names a file with none.
- The allowlist only shrinks. No change adds a line or raises a count.
