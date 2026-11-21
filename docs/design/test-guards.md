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

## Rule-set loads

- `//tools/ci:rule_set_loads_test` scans the `php`, `lua` and `qt` build-rule files named by `RULE_SETS` in `tools/ci/rule_set_loads_test.rs`: the public `defs.bzl` and `extensions.bzl`, `private/` rule code, toolchain, runtime and lock hub. Quality, coverage, tools, `lang.json` and tests are not rule-set files.
- It has two kinds:
  - `foreign-load`: a `load()` in a rule-set file of a label that is not in the same language, in `@rules_cc`, `@platforms`, `@bazel_skylib`, `@bazel_tools`, `@rules_shell`, `@bazel_lib`, `@llvm` or `@crates`, or in `RULES_DX_ALLOW`. `@rules_dx//x` and `//x` are one label and `:x` is resolved.
  - `reaches-rule-set`: a file in the `load()` closure of `RULES_DX_ALLOW` is a rule-set file. The failure prints the chain.
- `tools/ci/rule_set_loads_allowlist.txt` has lines `<path> <kind> <count>` under the same rules as the tags allowlist. It only shrinks.

## Determinism

- `//tools/testing:determinism_check` builds its patterns twice, each in a fresh output base with `--disk_cache=`, `--remote_cache=` and `--remote_executor=` empty, and compares the two compact execution logs with `dx_testing::exec_log`.
- With `--kinds` it adds one target per `entry_points` kind of every `<lang>/lang.json`: `bazel query --output=label_kind 'kind(...) except attr(tags, manual, //...)'` lists them, a `cquery` drops those with `IncompatiblePlatformProvider`, and the first label of each kind in sorted order is kept. The builds pass `--skip_incompatible_explicit_targets`.
- The `determinism` job of `.github/workflows/ci.yml` runs it with `bazel run`, passes Bazel with `--bazel`, and runs five shards with `--kinds --shard i/5`. `--shard` keeps the kind targets whose top-level package is every fifth in sorted order. Shard 0 adds `//appimage/...`, shard 1 `//cli/archive/...`, shard 2 `//android/private/packaging/...`, shard 3 `//msix/tests/...`, shard 4 `//deploy/private/...`.
- An action is keyed by `<label> <mnemonic> <output path>`. The job fails for an output that differs, for an action in one log only, and for an allowlist line for an action that does not differ or is in neither log.
- `tools/ci/determinism_allowlist.txt` lists the actions that differ. It only shrinks; `//tools/ci:determinism_allowlist_test` holds the cap.
- Measured on Linux with warm repository caches: 34 s and 33 s per build, 67 s for both.

## Analysis scaling

- `//tools/testing/deep_graph:defs.bzl` declares one linear chain per language: `go_library`, `java_library`, `kotlin_library`, `typescript_library`, `capnproto_library`, `julia_library` and `ruby_library`. `//tools/testing/deep_graph/d100` and `//tools/testing/deep_graph/d400` hold chains of depth 100 and 400. The targets are incompatible unless `--//tools/testing/deep_graph:deep_graph` is set.
- `//tools/testing:analysis_scaling_check` runs, for each depth in a fresh output base, `bazel build --nobuild` over the package with the aspects of `dx lint`, `dx typecheck`, `dx format` and `dx docs` and the `dx sync` aspects `dx_env_plan_aspect` and `dx_codegen_plan_aspect`. A warm-up run loads the packages; the measured run changes a `--define`, so it reanalyzes, and writes `--profile`. The analysis time runs from the `Load and analyze dependencies` phase marker to the next phase marker.
- It fails when depth 400 takes more than 8 or less than 2 times depth 100.
- The `analysis-scaling` job of `.github/workflows/ci.yml` runs it with `bazel run` and passes Bazel with `--bazel`.
- Measured on Linux: 61.5 s at depth 100, 204.6 s at depth 400, ratio 3.33 (ci.yml run 44524820323).

## In-graph prebuilt guard

- `//cli/prebuilt:guard_view` is a bool flag. When rules_dx is the root module and the flag is set, `@dx_prebuilt` and `@dx_gazelle` serve `//cli/prebuilt:guard_stub` for every tool they build from source. Its scope is `universal`, so it keeps its value in the exec configuration. `//cli/prebuilt:guard_view_test` checks that exec-configured hub tools resolve to the stub.
- `tools/testing/consumer/fixtures.bzl` splits `CONSUMER_FIXTURES` into `GO_GUARD_LANGS` and `JAVA_GUARD_LANGS`, held by the nested `go_prebuilt_guard_test` and `java_prebuilt_guard_test`, `IN_GRAPH_GUARD_LANGS`, `MACOS_IN_GRAPH_GUARD_LANGS`, and `GUARD_PENDING`. `//tools/testing/consumer:in_graph_prebuilt_guard_test` fails unless each fixture is in exactly one list.
- `JAVA_GUARD_LANGS` holds `java` only.
- The nested guards pin every registry key of each `//cli/prebuilt:target_tools` tool and write `dx_target_guard`, whose `targets` depend on the hub target of each such tool through a split transition over `@rules_dx//tools/platforms:<key>` for each of its keys. They fail when a target tool does not select its download for each key.
- `//tools/testing/consumer:in_graph_prebuilt_guard` writes, per `IN_GRAPH_GUARD_LANGS` language or per language of its space-separated `--langs` list, `check` and `docs` targets whose `deps` carry the `dx check` and `dx docs` aspects over the non-manual rules of `@dx_api_fixtures//<lang>/tests/consumer`, injects them with `--inject_repository`, and runs `bazel cquery 'deps(...)'` with the flag and `--lockfile_mode=error`.
- Each label is mapped to the form a consumer sees: the fixtures repo is the main repo, `@@//` is `@@rules_dx+//` and `@@+` is `@@rules_dx++`. The compiled binaries are checked against `tools/testing/consumer/prebuilt_guard_allowlist.txt` by the rules of `//tools/testing/consumer:prebuilt_guard`.
- Both guards drop a rules_dx binary whose every rule is a `java_binary` and whose `deps` hold no rules_dx `java_`, `kt_jvm_` or `scala_` rule with `srcs`. The in-graph guard maps each label back with `root_label` for that cquery.
- The `in-graph-prebuilt-guard` job of `.github/workflows/ci.yml` runs it with `bazel run` and passes Bazel with `--bazel`: on linux_x86_64 over `IN_GRAPH_GUARD_LANGS`, and on macos_arm64 with `--langs` set to `MACOS_IN_GRAPH_GUARD_LANGS`. `in_graph_prebuilt_guard_test` fails when the `langs` of the macos_arm64 matrix entry differ from that list.
