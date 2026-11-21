# Test guards

The guards check the rules of `docs/design/tests.md`.

## Snapshots

- `@dx_test_tags_sources//:sources.json` snapshots every `BUILD`, `BUILD.bazel` and `.bzl` file. Its repository rule watches every path it reads.
- `@dx_test_tags_sources//:files.json` snapshots `MODULE.bazel`, `<lang>/lang.json`, `<lang>/README.md`, repository `*.BUILD` and `*.BUILD.bazel` files and the `docs/` language and command pages. `@dx_test_tags_sources//:paths.json` lists every path under language directories.
- `@dx_test_tags_sources//:code.json` snapshots every `.rs`, `.sh` and `.tpl` file, `.bazelrc` and `.dx/bazelrc/*.bazelrc`.
- A test that walks the module tree declares the snapshots, sets `DX_SOURCES_JSON`, `DX_FILES_JSON` and `DX_PATHS_JSON`, and reads them with `dx_testing::source_snapshot()`. A Python test writes them into a scratch tree.

## Tags

- `//tools/ci:test_tags_test` scans `sources.json`.
- It counts `manual`, `exclusive` and the tags banned under Sandbox, remote execution and cache.
- It counts a tag in the `tags` of a test call, in `local = True` on one, in a loaded tags constant at each test call that uses it, and in a tags-named variable, a `"tags"` dict entry, or the arguments of a tags-named function.
- It counts `DX_UPSTREAM_TAG` in the `tags` of a test call as `manual`.
- A test call is a call whose name has the word `test`, `tests`, `suite`, `bench` or `fuzz`, is a `rule(test = True)`, or is a macro calling one of these.
- `tools/ci/test_tags_allowlist.txt` holds one line per file and tag: `<path> <tag> <count>`. Lines are sorted.
- The guard fails when a file has more occurrences than its line allows, or a line allows more than the file has or names a file with none.
- The allowlist only shrinks.

## Duplicates

- `//tools/ci:test_duplicates_test` scans `sources.json` for test calls in `BUILD` and `BUILD.bazel` files.
- Two calls in one file are duplicates when callee and arguments other than `name`, `tags` and `flaky` match. Arguments compare by source text without whitespace.
- It skips `test_suite`, calls that are not test calls, and calls with a positional or `**` argument.
- The guard fails with `path:line name` for each duplicate.
- It has no allowlist.

## Sandbox and host

- `//tools/ci:sandbox_escape_test`, `//tools/ci:host_env_test` and `//tools/ci:host_program_test` scan `sources.json`, `code.json` and the repository `*.BUILD` files of `files.json`. They read `DX_SOURCES_JSON`, `DX_FILES_JSON` and `DX_CODE_JSON` as maps, with no scratch tree.
- `//tools/ci:sandbox_escape_test` has three kinds:
  - `exec-requirement`: `requires-network`, `no-sandbox`, `no-remote`, `no-cache` or `local` as a key of execution requirements, in the `tags` of a call that is not a test call, or added by `--modify_execution_info`. `local` counts as a key only in an `execution_requirements` argument or a requirements-named variable.
  - `tmp-path`: a string or shell word that is `/tmp` or starts with `/tmp/`, and `env::temp_dir()` in Rust outside test files and `#[cfg(test)]` items.
  - `rc-flag`: `--incompatible_strict_action_env` or `--sandbox_default_allow_network=false` set by no line without a config in `.bazelrc` and `.dx/bazelrc/*.bazelrc`. The line is `.bazelrc rc-flag <missing flags>`.
- Section Checks of `docs/design/host-tools.md` lists the other two's kinds.
- Each has an allowlist `tools/ci/<name>_allowlist.txt` under the tags allowlist rules.
- They skip their own sources.

## Rule-set loads

- `//tools/ci:rule_set_loads_test` scans the `php`, `lua` and `qt` build-rule files named by `RULE_SETS` in `tools/ci/rule_set_loads_test.rs`: the public `defs.bzl` and `extensions.bzl`, `private/` rule code, toolchain, runtime and lock hub, not `private/defs.bzl`, quality, coverage, tools, `lang.json` or tests.
- It has three kinds:
  - `foreign-load`: a `load()` in a rule-set file of a label that is not in the same language, in `@rules_cc`, `@platforms`, `@bazel_skylib`, `@bazel_tools`, `@rules_shell`, `@bazel_lib`, `@llvm` or `@crates`, or in `RULES_DX_ALLOW`. `@rules_dx//x` and `//x` are one label and `:x` is resolved.
  - `foreign-label`: a label string outside `load()` in a rule-set file that names a `dx_` repo or a `rules_dx` package outside the rule set.
  - `reaches-rule-set`: a file in the `load()` closure of `RULES_DX_ALLOW` is a rule-set file. The failure prints the chain.
- `tools/ci/rule_set_loads_allowlist.txt` has lines `<path> <kind> <count>` under the same rules as the tags allowlist.

## Determinism

- `//tools/testing:determinism_check` builds its patterns twice, each in a fresh output base with `--disk_cache=`, `--remote_cache=` and `--remote_executor=` empty, and compares the two compact execution logs with `dx_testing::exec_log`.
- With `--kinds` it adds one target per `entry_points` kind of every `<lang>/lang.json`: `bazel query --output=label_kind 'kind(...) except attr(tags, manual, //...)'` lists them, a `cquery` drops those with `IncompatiblePlatformProvider`, and the first label of each kind in sorted order is kept. The builds pass `--skip_incompatible_explicit_targets`.
- The `determinism` job of `.github/workflows/ci.yml` runs it with `bazel run`, passes Bazel with `--bazel`, and runs six shards with `--kinds --shard i/6`. `--shard` keeps every sixth top-level package of kind targets. Shards 0, 1, 2, 3 and 5 add `//appimage/...`, `//cli/archive/...`, `//android/private/packaging/...`, `//msix/tests/...` and `//deploy/private/...`.
- A 40 min step cap under the 45 min job cap keeps a slow shard's log.
- Flags precede `--`, patterns follow, so `-//pkg/...` excludes.
- Outputs match between the logs by path; the `<label> <mnemonic>` before it only names the action. The job fails for an output that differs or is in one log only, and for an allowlist line whose output does not differ or is in neither log.
- `tools/ci/determinism_allowlist.txt` lists the actions that differ; `//tools/ci:determinism_allowlist_test` holds its cap.

## Analysis scaling

- `//tools/testing/deep_graph:defs.bzl` declares one linear chain per language: `go_library`, `java_library`, `kotlin_library`, `typescript_library`, `capnproto_library`, `julia_library` and `ruby_library`. `//tools/testing/deep_graph/d100` and `//tools/testing/deep_graph/d400` hold chains of depth 100 and 400. The targets are incompatible unless `--//tools/testing/deep_graph:deep_graph` is set.
- `//tools/testing:analysis_scaling_check` runs, for each depth in a fresh output base, `bazel build --nobuild` over the package with the aspects of `dx lint`, `dx typecheck`, `dx format` and `dx docs` and the `dx sync` aspects `dx_env_plan_aspect` and `dx_codegen_plan_aspect`. A warm-up run loads the packages; the measured run changes a `--define` and writes `--profile`. Analysis time runs from the `Load and analyze dependencies` phase marker to the next.
- It fails when depth 400 takes more than 8 or less than 2 times depth 100.
- The `analysis-scaling` job of `.github/workflows/ci.yml` runs it with `bazel run` and passes Bazel with `--bazel`.

## In-graph prebuilt guard

- `//cli/prebuilt:guard_view` is a bool flag. When rules_dx is the root module and the flag is set, `@dx_prebuilt`, `@dx_gazelle` and `@php_launcher` serve `//cli/prebuilt:guard_stub` for every tool they build from source. Its `universal` scope keeps it in the exec configuration. `//cli/prebuilt:guard_view_test` checks that exec-configured hub tools resolve to the stub.
- `tools/testing/consumer/fixtures.bzl` splits `CONSUMER_FIXTURES` into `GO_GUARD_LANGS` and `JAVA_GUARD_LANGS`, held by the nested `go_prebuilt_guard_test` and `java_prebuilt_guard_test`, `IN_GRAPH_GUARD_LANGS`, `MACOS_IN_GRAPH_GUARD_LANGS`, `WINDOWS_IN_GRAPH_GUARD_LANGS` and `GUARD_PENDING`. `//tools/testing/consumer:in_graph_prebuilt_guard_test` fails unless each fixture is in exactly one list.
- The nested guards pin every registry key of each `//cli/prebuilt:target_tools` tool and write `dx_target_guard`, whose `targets` depend on the hub target of each such tool through a split transition over `@rules_dx//tools/platforms:<key>` for each of its keys. They fail when a target tool does not select its download for each key.
- `//tools/testing/consumer:in_graph_prebuilt_guard` writes, per `IN_GRAPH_GUARD_LANGS` language or per language of its space-separated `--langs` list, `check` and `docs` targets whose `deps` carry the `dx check` and `dx docs` aspects over the non-manual rules of `@dx_api_fixtures//<lang>/tests/consumer`, injects them with `--inject_repository`, and runs `bazel cquery 'deps(...)'` with the flag and `--lockfile_mode=error`.
- Each label is mapped to the form a consumer sees: the fixtures repo is the main repo, `@@//` is `@@rules_dx+//` and `@@+` is `@@rules_dx++`. The compiled binaries are checked against `tools/testing/consumer/prebuilt_guard_allowlist.txt` by the rules of `//tools/testing/consumer:prebuilt_guard`.
- Both guards drop each binary of a repo named by a `repo <canonical repo> <lang>...` allowlist line, a `rules_dx++dx_pins+rules_*` upstream rule set, for a consumer of one of its languages; an unused line is stale.
- Both guards drop a rules_dx binary whose every rule is a `java_binary` and whose `deps` hold no rules_dx `java_`, `kt_jvm_` or `scala_` rule with `srcs`. The in-graph guard maps each label back with `root_label` for that cquery.
- The `in-graph-prebuilt-guard` job of `.github/workflows/ci.yml` runs it with `bazel run --bazel`: on linux_x86_64 over `IN_GRAPH_GUARD_LANGS`, and on macos_arm64 and windows_x86_64 with `--langs` set to `MACOS_IN_GRAPH_GUARD_LANGS` and `WINDOWS_IN_GRAPH_GUARD_LANGS`. `in_graph_prebuilt_guard_test` fails when a leg's `langs` differ from its list.
