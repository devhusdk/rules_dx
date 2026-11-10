# Nested tests design

This page fixes which tests may run Bazel inside a test, and how they run. Conversions follow it.

## Kinds

- A unit test checks code with no Bazel. It covers what `dx` hands to Bazel: a fake Bazel records the argv.
- An analysis test checks providers, actions and failures of a rule.
- An in-graph fixture is a target in the main module that the default test set builds and runs.
- A nested test starts a Bazel server inside a test.
- A nested test is allowed only for a claim about a consumer workspace that the main module cannot make: module resolution of a consumer, a release archive, `dx init` plus `dx generate` on a tree with no BUILD files.
- A claim about what a rule builds is an analysis test or an in-graph fixture.
- A claim about what `dx` passes to Bazel is a unit test.
- What Bazel does with a flag is not tested here.

## Floor

Measured on Linux x86_64 with the pinned Bazel 9.3.1, an empty module and one write action:

| Step | Time |
| --- | --- |
| Server start, install base extracted | 1.3 s |
| Server start, install base present | 0.9 s |
| Module resolution | 5.7 s |
| Analysis and execution | 0.3 s |
| Second build, same server | 0.04 s |

- A cold nested test takes 7 seconds before it loads rules_dx. No nested test runs under 1 second.
- The 1 second target is met by a nested test only as a cached result.
- A test that must run under 1 second uncached is converted.

## Decisions

| Test | Proves | Decision |
| --- | --- | --- |
| `//cli/cli/tests/cli_argv_e2e:cli_argv_e2e_test` | `dx test` forwards `--test_arg` | Converted to a unit test in `//cli/cli:dx_cli_test` |
| `//tools/testing/starters:*` | `dx init` plus a starter fixture gives the listed files and `bazel_dep` names | One in-graph test over all fixtures, no `manual` |
| `//tools/testing/hermetic:*` golden checks | The consumer manifest matches its golden | In-graph already. Stay |
| `//tools/testing/hermetic:c_fixture_*`, `cpp_fixture_*` | The selected compiler built the binary | In-graph fixtures per toolchain that check the producer string |
| `//tools/testing/hermetic:cpp_rust` | A consumer fetches, builds and tests with no install and no network | Stays nested, on the harness |
| `//tools/testing/hermetic:isolation_probe_test` | The isolation layer denies network and host tools | Deleted with the isolation layer |
| `//tools/testing/hermetic:*_maven_pin` | The Maven lock matches a fresh pin | `bazel run` update tools. `consumer_maven_lock_test` stays |
| `//tools/testing/hermetic:cuda_gpu_nested_test` | A GPU test runs through a consumer | Deleted. `cuda_gpu_facts_test` stays |
| `//tools/testing:nested_bazel_harness_test` | The harness runs the pinned Bazel offline and stops its server | Stays nested |
| `//core/quality/tests:quality_check_smoke_test` | `dx check` reports each phase and diagnostic | Unit test for the events, in-graph fixtures for the diagnostics |
| `//deploy/private/release/tests:release_candidate_smoke_test` | A release archive works through a registry with no override | Stays nested, on the harness |
| `//deploy/private/release/tests:readme_quickstart_smoke_test` | The README quickstart and `bcr_test_module` run as written | Stays nested, in the same target as the release candidate |
| `//examples/adopt-polyglot:consumer_test` | `dx init` and `dx generate` adopt a tree with no BUILD files, twice with no change | Stays nested, on the harness |

- `tools/docs/support_matrix` lists the in-graph test of a language once its nested test is converted.
- `tools/testing/nested_smoke.txt` lists every nested test. The cap in `nested_smoke_guard.sh` only goes down.
- A nested test gets its cold time in `nested_smoke.txt` when it passes on the harness.

## Harness

- `nested_bazel_test` in `//tools/testing/hermetic:defs.bzl` is the only way to declare a nested test.
- One test target starts one server. Its cases share the server and run on one thread.
- The server stops when the test binary exits.

### Bazel

- Bazel is `//tools/testing:nested_bazel_bin`, the binary pinned by sha256 for the `.bazelversion` release, read from runfiles.
- The harness starts it through `//tools/testing:nested_bazel_shim`.
- No bazelisk. No `DX_NESTED_BAZEL`. No lookup on `PATH`.

### Environment

- The test sets `PATH=/dx_no_path` and inherits no host variable. `NESTED_BAZEL_ENV_INHERIT` is deleted.
- `HOME`, the output user root and every workspace are under `TEST_TMPDIR`.
- No slot root on the host. `DX_NESTED_BAZEL_ROOT`, `DX_NESTED_BAZEL_DISK_CACHE` and `DX_NESTED_BAZEL_REPOSITORY_CACHE` are deleted.
- The nested Bazel reads one rc file that the harness writes. It reads no home, system or workspace rc of the host.
- The nested Bazel uses no disk cache and no remote cache.
- Nested actions run with `--spawn_strategy=local`. The test sandbox is the boundary.

### Repository inputs

- rules_dx reaches the consumer through `local_path_override` on the `module_srcs` runfiles.
- A consumer has a checked-in `MODULE.bazel.lock`. The nested Bazel runs with `--lockfile_mode=error`.
- Every file the consumer downloads is a test input: registry files, module archives, toolchain archives.
- `nested_repository_cache` declares them. It reads a checked-in list of URL and sha256, one `http_file` per entry, and gives a manifest of sha256 to runfile.
- The harness builds a repository cache under `TEST_TMPDIR` from the manifest and passes `--repository_cache`.
- A nested test lists the cache sets of the languages it builds, nothing more.
- The downloader config blocks every host, `bcr.bazel.build` included.
- A missing entry fails the test with the URL. The list is regenerated by a `bazel run` tool, never by hand.

### Tags

- No `local`, `no-sandbox`, `no-cache`, `no-remote`, `external`, `requires-network` or `manual`.
- `NESTED_BAZEL_NETWORK_TAGS`, `NESTED_BAZEL_NETWORK_ENV`, `STARTER_CONSUMER_TAGS` and `QUALIFICATION_TAGS` are deleted.
- A nested test keeps `resources:nested_bazel:1`, `resources:cpu:2` and `resources:memory:2560`.
- The result is cached like any other test. Its inputs are the pinned Bazel, the shim, `module_srcs`, the cache sets and the fixture.

### Size

- A nested test is `size = "large"` with `timeout = "moderate"`.
- A nested test that runs past 60 seconds cold is split or converted.
