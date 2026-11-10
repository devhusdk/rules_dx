# Layout design

This page fixes the repository layout. Directory moves, module files and generators follow it.

## Modules

- The repo is one Bazel module, `rules_dx`. Users add one `bazel_dep`.
- Language rule sets are `bazel_dep`s of `rules_dx` at their `# floor:` versions. Bazel fetches a rule set only when a loaded `.bzl` file needs it.
- Core never loads a language `.bzl` file. One exception: `core/quality/tools.bzl` loads the `_tools.bzl` files. A test enforces it.
- dx loads a language's quality aspects only when the workspace has that language's files.
- There are no family modules. A family is a grouping label in the language manifest, used for CI sharding and docs.

## Language directory

Each language owns one top-level directory.

```text
<lang>/defs.bzl          public rules and macros
<lang>/extensions.bzl    public module extensions
<lang>/BUILD.bazel       toolchain_type and public config flags
<lang>/lang.json         manifest
<lang>/pins.json         tool pins
<lang>/private/          rules, toolchains, aspects, quality, coverage, tools
<lang>/tests/            tests
```

- `defs.bzl` is the only file users load.
- `extensions.bzl` holds the public module extensions.
- `private/` holds everything else. Users never load from it.
- `tests/` holds the language tests and fixtures.
- New language code goes in `<lang>/`.

## Labels

- Rules and macros: `@rules_dx//<lang>:defs.bzl`.
- Toolchain type: `@rules_dx//<lang>:toolchain_type`.
- Public config flags are targets in `<lang>/BUILD.bazel`.
- No label points into `private/`.

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
third_party/ vendored upstream sources
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
- `patches/` holds upstream patches. The patch table is generated from `MODULE.bazel`.
- `third_party/` holds vendored upstream sources.
- `docs/` holds user docs. Per-language pages are generated from `lang.json`.
- Every example in `examples/` is a tested target.
- The root holds only language directories with a `lang.json` and the directories above. A test enforces the list, and the list has no pending entries.
- A directory that holds only a README is not a root directory. Its page lives in `docs/`.

## Quality code

- `core/quality/` holds the framework: runner, adapters, tiers, the tool table and the aspect factory.
- A language's aspects, tool rows and tool packages live in `<lang>/private/quality/`.
- A file family that has no rules gets no root directory. Markdown, CSS, GraphQL, YAML, JSON, TOML, XML, shell and plain text are such families.
- The aspects and tool rows of such a family's tools that do not run on Node stay in `core/quality/families/`. Its Node tools follow the Node tools section.
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
- Every Node tool has one target in `javascript/private/quality/bin/`. That covers Prettier, remark, Stylelint, graphql-eslint, gplint, Redocly, Asciidoctor, TypeDoc, esbuild, Wrangler, API Extractor and protoc-gen-es.
- `javascript/private/quality/js.bzl` defines the `js` shard: the `real_js_*` aspects and probes. The shard runs every Node tool in the tool table.
- `javascript/private/quality/js_tools.bzl` holds the rows of Biome, ESLint, remark, Stylelint and `tsc`. The rows of Prettier, graphql-eslint, gplint and Redocly are in `core/quality/families/core_tools.bzl`.
- `typescript/private/quality/` holds the `tsc` aspect and its report. Its tool defaults point at the hub.
- The manifests of javascript, typescript, astro, mdx, svelte and vue name `javascript/private/quality:js.bzl` as their quality file.
- Another directory uses a hub tool through the label `//javascript/private/quality/bin:<tool>` as an attribute default. It does not copy the entry point.

## Language manifest

`<lang>/lang.json` is the single description of a language.

- Its fields are `name`, `title`, `family`, `ecosystems`, `manifests`, `lockfiles`, `tools`, `platforms`, `pins` and `entry_points`.
- Every manifest validates against one shared schema. A test checks it.
- It drives quality registration, the dx ecosystem registry, `dx init`, the support matrix and the per-language docs.
- Family values only group languages. They create no module, directory or label.
- Code reads the manifest. It does not repeat its values.

## Tool pins

`<lang>/pins.json` holds the pinned tools.

- Each pin records the `sha256` and the urls.
- One shared extension in `core/` reads every `pins.json`.
- `dx update` rewrites the files. Nobody edits them by hand.
- The manifest `pins` field names the language's pin files.
- A language never ships its own pin-reading extension.

## Generated files

- A generated file carries a diff test against its generator.
- Never edit a generated file by hand. Change the source and regenerate.
- The generators for `lang.json` and `pins.json` outputs live in `tools/`.
