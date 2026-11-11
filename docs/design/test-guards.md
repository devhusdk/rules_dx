# Test guards

The guards check the rules of `docs/design/tests.md`.

## Snapshots

- `@dx_test_tags_sources//:sources.json` snapshots every `BUILD`, `BUILD.bazel` and `.bzl` file. Its repository rule watches every path it reads.
- `@dx_test_tags_sources//:files.json` snapshots `MODULE.bazel`, `<lang>/lang.json`, `<lang>/README.md`, repository `*.BUILD` and `*.BUILD.bazel` files and the `docs/` language and command pages. `@dx_test_tags_sources//:paths.json` lists every path under language directories.
- `@dx_test_tags_sources//:code.json` snapshots every `.rs`, `.sh` and `.tpl` file, `.bazelrc` and `.dx/bazelrc/*.bazelrc`.
- A test that walks the module tree declares the snapshots, sets `DX_SOURCES_JSON`, `DX_FILES_JSON` and `DX_PATHS_JSON`, and reads them with `dx_testing::source_snapshot()`. A Python test writes them into a scratch tree.

## Tags

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

## Duplicates

- `//tools/ci:test_duplicates_test` scans `sources.json` for test calls in `BUILD` and `BUILD.bazel` files.
- Two calls in one file are duplicates when they have the same callee and the same arguments other than `name`, `tags` and `flaky`. Arguments compare by source text without whitespace.
- It skips `test_suite`, calls that are not test calls, and calls with a positional or `**` argument.
- The guard fails with `path:line name` for each duplicate.
- It has no allowlist. A duplicate is deleted.

## Sandbox and host

- `//tools/ci:sandbox_escape_test`, `//tools/ci:host_env_test` and `//tools/ci:host_program_test` scan `sources.json`, `code.json` and the repository `*.BUILD` files of `files.json`. They read `DX_SOURCES_JSON`, `DX_FILES_JSON` and `DX_CODE_JSON` as maps and write no scratch tree.
- `//tools/ci:sandbox_escape_test` has three kinds:
  - `exec-requirement`: `requires-network`, `no-sandbox`, `no-remote`, `no-cache` or `local` as a key of execution requirements, in the `tags` of a call that is not a test call, or added by `--modify_execution_info`. `local` counts as a key only in an `execution_requirements` argument or a requirements-named variable.
  - `tmp-path`: a string or shell word that is `/tmp` or starts with `/tmp/`, and `env::temp_dir()` in Rust outside test files and `#[cfg(test)]` items.
  - `rc-flag`: `--incompatible_strict_action_env` or `--sandbox_default_allow_network=false` set by no line without a config in `.bazelrc` and `.dx/bazelrc/*.bazelrc`. The line is `.bazelrc rc-flag <missing flags>`.
- `docs/design/host-tools.md` section Checks lists the kinds of the other two.
- Each has its own allowlist, `tools/ci/<name>_allowlist.txt`, with lines `<path> <kind> <count>` under the same rules as the tags allowlist.
- The three guards skip their own sources.

## Determinism

- `//tools/testing:determinism_check` builds `//appimage/...`, `//cli/archive/...` and, with `--kinds`, every non-manual target from `bazel query 'kind(<entry_points of every <lang>/lang.json>, //...)'` twice, each in a fresh output base with `--disk_cache=`, `--remote_cache=` and `--remote_executor=` empty, and compares the two compact execution logs with `dx_testing::exec_log`.
- The `determinism` job of `.github/workflows/ci.yml` runs it with `bazel run` and passes Bazel with `--bazel`.
- An action is keyed by `<label> <mnemonic> <output path>`. The job fails for an output that differs, for an action in one log only, and for an allowlist line that no longer differs or is in neither log.
- `tools/ci/determinism_allowlist.txt` lists the actions that differ. It only shrinks; `//tools/ci:determinism_allowlist_test` holds the cap.
- Measured on Linux with warm repository caches: 34 s and 33 s per build, 67 s for both.
