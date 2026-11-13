# Layout design

This page fixes the repository layout.

## Modules

- The repo is one Bazel module, `rules_dx`. Users add one `bazel_dep`.
- Language rule sets are `bazel_dep`s of `rules_dx` at their `# floor:` versions. Bazel fetches a rule set only when a loaded `.bzl` file needs it.
- Core never loads a language `.bzl` file. One exception: `core/quality/tools.bzl` loads the `_tools.bzl` files. A test enforces it.
- dx loads a language's quality aspects only when the workspace has that language's files.
- `rules_php`, `rules_lua` and `rules_qt` are rule-set modules that rules_dx owns. rules_dx depends on each like any upstream rule set, and each depends on rules_dx as a normal `bazel_dep`.
- A rule-set module holds the build rules: toolchain, `<lang>_library`, `<lang>_binary`, `<lang>_test`, `<lang>_bench`, providers, runtime pins and the dependency hub. Its public API is `@rules_<lang>//<lang>:defs.bzl` and `@rules_<lang>//<lang>:extensions.bzl`.
- `<lang>/` in rules_dx keeps the dx layer: `lang.json`, quality, coverage, docs, Gazelle and CLI ecosystem code. `<lang>/private/defs.bzl` wraps the rule-set macros, as `zig/private/defs.bzl` wraps `@rules_zig`.
- A rule-set module loads from rules_dx only `@rules_dx//rust:defs.bzl` and the files in `RULES_DX_ALLOW` of `tools/ci/rule_set_loads_test.rs`.
- rules_dx's floor on each rule-set module is `equals tested`. It moves with each rules_dx release.
- There are no family modules. A family is a grouping label in the language manifest, used for CI sharding and docs.

## Language directory

Each language owns one top-level directory.

```text
<lang>/defs.bzl          public rules and macros
<lang>/extensions.bzl    public module extensions
<lang>/BUILD.bazel       toolchain_type and public config flags
<lang>/lang.json         manifest
<lang>/pins.json         tool pins
<lang>/README.md         guide
<lang>/private/          rules, toolchains, aspects, quality, coverage, tools
<lang>/tests/            tests
```

- `defs.bzl` is the only file users load.
- `extensions.bzl` holds the public module extensions.
- A module extension is named after its language (`qt`, `ocaml`). An extension that serves one sub-feature is named after that feature (`cuda_license`, `zig_deps`).
- `private/` holds everything else. Users never load from it.
- `tests/` holds the language tests and fixtures. Starlark test suites and `testdata/` live there, instantiated from `tests/BUILD.bazel`, not in `private/`.
- Starlark suites are named `tests/defs_tests.bzl` for rule and macro analysis tests, `tests/failure_tests.bzl` for expected analysis failures, and `tests/toolchain_tests.bzl` for pin and hub checks.
- New language code goes in `<lang>/`.

## Labels

- Rules and macros: `@rules_dx//<lang>:defs.bzl`.
- Toolchain type: `@rules_dx//<lang>:toolchain_type`.
- Public config flags are targets in `<lang>/BUILD.bazel`.
- No label points into `private/`.

## Executable macro signatures

This rule covers every public `<lang>_binary`, `<lang>_test`, `<lang>_bench` and `<lang>_fuzz_test` macro in `<lang>/private/defs.bzl`.

```text
<lang>_<kind>(name, *, srcs = None, main = None, <other parameters>, **kwargs)
```

- `name` is the only positional parameter. Every other parameter is keyword-only.
- `srcs` is the first keyword parameter of a macro that takes it.
- `srcs` defaults to `None` in binary and test macros. A bench or fuzz macro may require it.
- `main` names the entry file. It follows `srcs` and defaults to `None`.
- No macro takes `entry_point`. JVM macros take `main_class` or `target_class`.
- `visibility` defaults to `None`.
- `**kwargs` comes last.
- `tools/testing/signatures/macro_signatures.py` checks the parameter order, the defaults and the absence of `entry_point`. It lists the macros that require `srcs` or `main`.

## Shared directories

```text
core/       shared Starlark
config/     public build settings
cli/        the dx CLI
dx/         command targets consumers run
deploy/     publishing rules
gazelle/    shared Gazelle plugins
ci/         CI templates consumers include by URL
tools/      maintainer-only scripts and generators
patches/    upstream patches
third_party/ dependency manifests and lock files
bcr_test_module/ the BCR presubmit module
docs/       user docs
examples/   tested examples
```

- `core/` holds providers, the quality framework, `dx_wrap`, and the toolchain and pins factory. It holds no language code.
- `config/` holds public build settings that are not specific to one language.
- `core/private/` holds core internals.
- `cli/` holds every Rust crate.
- `dx/` holds only a `BUILD.bazel` with the targets `dx` runs in a consumer workspace. It holds no `.bzl` file.
- `deploy/` follows the language directory shape without a manifest: `deploy/defs.bzl`, `deploy/private/`, `deploy/tests/`.
- `gazelle/` holds the shared Gazelle plugins.
- `ci/` holds one directory per CI system. Its file paths are public URLs.
- `tools/` holds scripts that only maintainers run, and the generators.
- WASI is a Rust target, not a language. Its test helpers `wasi_run` and `wasmtime` live in `//tools/testing/wasi`.
- `patches/` holds upstream patches and the rules_dx registry. The patch table is generated from them.
- `third_party/` holds dependency manifests and lock files.
- `docs/` holds user docs. Per-language pages are generated from `lang.json`.
- Every example in `examples/` is a tested target.
- The root holds only language directories with a `lang.json` and the directories above. A test enforces the list, which has no pending entries.
- A directory that holds only a README is not a root directory. Its page lives in `docs/`.
- Every `.bzl` file is loaded, every fixture and Rust file is named by a target or a test, every `filegroup` and exported file has a user, and every directory belongs to a package. `//tools/ci:unreferenced_files_test` enforces it.

## Quality code

- `core/quality/` holds the framework: runner, adapters, tiers, the tool table and the aspect factory.
- A language's aspects, tool rows and tool packages live in `<lang>/private/quality/`.
- A file family with no rules, such as Markdown, CSS, GraphQL, YAML, JSON, TOML, XML, shell or plain text, gets no root directory.
- Such a family's aspects and tool rows for tools not run on Node stay in `core/quality/families/`. Its Node tools follow the Node tools section.
- A shard that serves several languages lives with the language that owns its runtime: `js` in `javascript/`, `jvm` in `java/`, `beam` in `erlang/`, `dotnet` in `dotnet/`, `cc` in `cc/`.
- A tool has all its rows in one `_tools.bzl` table. Moving a tool binary never moves its rows.
- A `<name>_tools.bzl` file is data. It loads only the core data files `//core/quality:tool_defs.bzl` and `//core/quality:taxonomy.bzl`.
- `core/quality/tools.bzl` loads every `_tools.bzl` file. It is the only core file that loads from a language directory.
- A tier package `core/quality/tiers/<tool>/` loads only core `.bzl` files.
- A tool shipped as a prebuilt binary or bundle has no runtime owner. Its package stays in `core/quality/tools/`.

## Node tools

- `javascript/` owns every tool that runs on Node, whichever file family the tool checks.
- There is one tool hub, `@dx_npm_tools`. Its `package.json`, `pnpm-lock.yaml` and launcher live in `javascript/private/quality/`.
- No other directory holds a tool `package.json`, a tool lockfile or a second `npm_translate_lock` for tools.
- Every Node tool has one target in `javascript/private/quality/bin/`.
- `javascript/private/quality/js.bzl` defines the `js` shard: the `real_js_*` aspects and probes. It runs every Node tool in the tool table.
- `javascript/private/quality/js_tools.bzl` holds the rows of Biome, ESLint, remark, Stylelint and `tsc`. The rows of Prettier, graphql-eslint, gplint and Redocly are in `core/quality/families/core_tools.bzl`.
- `typescript/private/quality/` holds the `tsc` aspect and its report. Its tool defaults point at the hub.
- The manifests of javascript, typescript, astro, mdx, svelte and vue name `javascript/private/quality:js.bzl` as their quality file.
- Another directory uses a hub tool through `//javascript/private/quality/bin:<tool>` as an attribute default and copies no entry point.

## Language manifest

`<lang>/lang.json` is the single description of a language.

- Its fields are `name`, `title`, `family`, `ecosystems`, `manifests`, `lockfiles`, `tools`, `platforms`, `pins`, `entry_points` and `setup`.
- Every manifest validates against one shared schema. A test checks it.
- It drives quality registration, the dx ecosystem registry, `dx init`, the support matrix and the per-language docs.
- Its `quality` entry names the aspect file, aspects and probes. dx loads the file when the workspace has a file with one of `file_extensions` or named one of `file_names` alone or before a dot, or always when `every_workspace` is set.
- `cli/families/languages.json` is generated from the `quality` and `docs` entries. The dx plan registry reads it.
- Family values only group languages. They create no module, directory or label.
- Code reads the manifest. It does not repeat its values.

## Tool pins

`<lang>/pins.json` holds the pinned tools.

- Each pin records the `sha256` and the urls.
- One shared extension in `core/` reads every `pins.json`.
- `dx update` rewrites the files. Nobody edits them by hand.
- The manifest `pins` field names the language's pin files.
- A language never ships its own pin-reading extension. A rule-set module reads its own `pins.json` with its own extension, and `dx update` rewrites it in that module's repo.
- `//tools/ci:pin_locations_test` guards this. `tools/ci/pin_locations_allowlist.txt` lists what is left and only shrinks.

## Generated files

- A generated file carries a diff test against its generator.
- Never edit a generated file by hand. Change the source and regenerate.
- The generators for `lang.json` and `pins.json` outputs live in `tools/`.
