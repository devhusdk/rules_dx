# Test guards

The guards check the rules of `docs/design/tests.md`.

## Snapshots

- `@dx_test_tags_sources//:sources.json` snapshots every `BUILD`, `BUILD.bazel` and `.bzl` file. Its repository rule watches every path it reads.
- `@dx_test_tags_sources//:files.json` snapshots `MODULE.bazel`, `<lang>/lang.json`, `<lang>/README.md`, repository `*.BUILD` and `*.BUILD.bazel` files and the `docs/` language and command pages.
- A test that walks the module tree declares the snapshots, sets `DX_SOURCES_JSON`, `DX_FILES_JSON` and `DX_PATHS_JSON`, and reads them with `dx_testing::source_snapshot()`. A Python test writes them into a scratch tree.

## Determinism

- `//tools/testing:determinism_check` builds its patterns twice, each in a fresh output base with no disk or remote cache, and compares the two compact execution logs with `dx_testing::exec_log`.
- With `--kinds` it adds one target per `entry_points` kind of every `<lang>/lang.json`: a `query` lists the non-manual ones, a `cquery` drops those with `IncompatiblePlatformProvider`, and the first label of each kind is kept. The builds pass `--skip_incompatible_explicit_targets`.
- The `determinism` job of `.github/workflows/ci.yml` runs it with `bazel run` in seven shards with `--kinds --shard i/7`, which keeps every seventh top-level package of kind targets. Shards 0 to 3 add `//appimage/...`, `//cli/archive/...`, `//android/private/packaging/...` and `//msix/tests/...`; shards 5 and 6 split `//deploy/private/...` into `:all` and its subpackages.
- Each build deletes its output base after `shutdown`.
- A 40 min step cap keeps a slow shard's log.
- Flags precede `--`, patterns follow, so `-//pkg/...` excludes.
- Outputs match between the logs by path; `<label> <mnemonic>` only names the action. The job fails for an output that differs or is in one log only, and for a stale allowlist line.
- `tools/ci/determinism_allowlist.txt` lists the actions that differ.

## Analysis scaling

- `//tools/testing/deep_graph:defs.bzl` declares one linear chain per language: `go_library`, `java_library`, `kotlin_library`, `typescript_library`, `capnproto_library`, `julia_library` and `ruby_library`. `//tools/testing/deep_graph/d100` and `//tools/testing/deep_graph/d400` hold chains of depth 100 and 400. The targets are incompatible unless `--//tools/testing/deep_graph:deep_graph` is set.
- `//tools/testing:analysis_scaling_check` runs, for each depth in a fresh output base, `bazel build --nobuild` over the package with the aspects of `dx lint`, `dx typecheck`, `dx format` and `dx docs` and the `dx sync` aspects `dx_env_plan_aspect` and `dx_codegen_plan_aspect`. A warm-up run loads the packages; the measured run changes a `--define` and writes `--profile`. Analysis time runs from the `Load and analyze dependencies` phase marker to the next.
- It fails when depth 400 takes more than 8 or less than 2 times depth 100.
- The `analysis-scaling` job of `.github/workflows/ci.yml` runs it with `bazel run` and passes Bazel with `--bazel`.

## In-graph prebuilt guard

- `//cli/prebuilt:guard_view` is a bool flag. When rules_dx is the root module and the flag is set, `@dx_prebuilt`, `@dx_gazelle`, `@php_launcher` and `@qt_launcher` serve `//cli/prebuilt:guard_stub` for every tool they build from source. Its `universal` scope keeps it in the exec configuration. `//cli/prebuilt:guard_view_test` checks that exec-configured hub tools resolve to the stub.
- `tools/testing/consumer/fixtures.bzl` splits `CONSUMER_FIXTURES` into `GO_GUARD_LANGS` and `JAVA_GUARD_LANGS`, held by the go and java guard cases of the nested, sharded `network_test`, `IN_GRAPH_GUARD_LANGS`, `MACOS_IN_GRAPH_GUARD_LANGS`, `WINDOWS_IN_GRAPH_GUARD_LANGS` and `GUARD_PENDING`. `//tools/testing/consumer:in_graph_prebuilt_guard_test` fails unless each fixture is in exactly one list.
- The nested guards pin every registry key of each `//cli/prebuilt:target_tools` tool and write `dx_target_guard`, whose `targets` depend on the hub target of each such tool through a split transition over `@rules_dx//tools/platforms:<key>` for each of its keys. They fail when a target tool does not select its download for each key.
- `//tools/testing/consumer:in_graph_prebuilt_guard` writes, per `IN_GRAPH_GUARD_LANGS` language or per language of its space-separated `--langs` list, `check` and `docs` targets whose `deps` carry the `dx check` and `dx docs` aspects over the non-manual rules of `@dx_api_fixtures//<lang>/tests/consumer`, injects them with `--inject_repository`, and runs `bazel cquery 'deps(...)'` with the flag and `--lockfile_mode=error`.
- Each label is mapped to the form a consumer sees: the fixtures repo is the main repo, `@@//` is `@@rules_dx+//` and `@@+` is `@@rules_dx++`. The compiled binaries are checked against `tools/testing/consumer/prebuilt_guard_allowlist.txt` by the rules of `//tools/testing/consumer:prebuilt_guard`.
- Both guards drop each binary of a repo named by a `repo <canonical repo> <lang>...` allowlist line, a `rules_dx++dx_pins+rules_*` upstream rule set, for a consumer of one of its languages; an unused line is stale.
- Both guards drop a rules_dx binary whose every rule is a `java_binary` and whose `deps` hold no rules_dx `java_`, `kt_jvm_` or `scala_` rule with `srcs`. The in-graph guard maps each label back with `root_label` for that cquery.
- The `in-graph-prebuilt-guard` job of `.github/workflows/ci.yml` runs it with `bazel run --bazel`: on linux_x86_64 over `IN_GRAPH_GUARD_LANGS`, and on macos_arm64 and windows_x86_64 with `--langs` set to `MACOS_IN_GRAPH_GUARD_LANGS` and `WINDOWS_IN_GRAPH_GUARD_LANGS`.
