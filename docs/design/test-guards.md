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

## Sandbox and host

- `//tools/ci:sandbox_escape_test`, `//tools/ci:host_env_test` and `//tools/ci:host_program_test` scan `sources.json`, `code.json` and the repository `*.BUILD` files of `files.json`. They read `DX_SOURCES_JSON`, `DX_FILES_JSON` and `DX_CODE_JSON` as maps and write no scratch tree.
- `//tools/ci:sandbox_escape_test` has three kinds:
  - `exec-requirement`: `requires-network`, `no-sandbox`, `no-remote`, `no-cache` or `local` as a key of execution requirements, in the `tags` of a call that is not a test call, or added by `--modify_execution_info`. `local` counts as a key only in an `execution_requirements` argument or a requirements-named variable.
  - `tmp-path`: a string or shell word that is `/tmp` or starts with `/tmp/`, and `env::temp_dir()` in Rust outside test files and `#[cfg(test)]` items.
  - `rc-flag`: `--incompatible_strict_action_env` or `--sandbox_default_allow_network=false` set by no line without a config in `.bazelrc` and `.dx/bazelrc/*.bazelrc`. The line is `.bazelrc rc-flag <missing flags>`.
- `docs/design/host-tools.md` section Checks lists the kinds of the other two.
- Each has its own allowlist, `tools/ci/<name>_allowlist.txt`, with lines `<path> <kind> <count>` under the same rules as the tags allowlist.
- The three guards skip their own sources.
