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

- `exclusive` is allowed only where two tests cannot share a resource.
- Each case has a line in the allowlist that names the resource as `resource=<name>`.
- A resource that Bazel can count is declared with a `resources:<name>:<n>` tag instead.

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
- A fixture is a library, binary or file target. A test target is never the fixture of another test.

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

## Duplicates

- A test that proves what another test already proves is deleted.
- A test that cannot fail is deleted.

## Guard

- `//tools/ci:test_tags_test` scans every `BUILD`, `BUILD.bazel` and `.bzl` file.
- It counts the banned tags `manual`, `local`, `no-sandbox`, `no-cache`, `no-remote`, `external`, `requires-network` and `exclusive`.
- It counts a tag in the `tags` of a test rule or test macro call, in `local = True` on such a call, and in a tags value built outside a call: a tags-named variable, a `"tags"` dict entry, or the arguments of a tags-named function.
- It counts a loaded tags constant at each test call that uses it.
- A call is a test call when its name has the word `test`, `tests`, `suite`, `bench` or `fuzz`, when the name is a `rule(test = True)`, or when the name is a macro that calls one of these.
- It skips `tags` on a call that is not a test call.
- `tools/ci/test_tags_allowlist.txt` holds one line per file and tag: `<path> <tag> <count>`. An `exclusive` line ends with `resource=<name>`. Lines are sorted.
- The guard fails when a file has more occurrences than its line allows.
- The guard fails when a line allows more than the file has, or names a file with none.
- The allowlist only shrinks. No change adds a line or raises a count.
