# Layout design

This page fixes the repository layout. Directory moves, module files and generators follow it.

## Modules

- The repo is one Bazel module, `rules_dx`. Users add one `bazel_dep`.
- Language rule sets are `bazel_dep`s of `rules_dx` at their `# floor:` versions. Bazel fetches a rule set only when a loaded `.bzl` file needs it.
- Core never loads a language `.bzl` file. A test enforces it.
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
gazelle/    shared Gazelle plugins
tools/      maintainer-only scripts and generators
patches/    upstream patches
third_party/ vendored upstream sources
docs/       user docs
examples/   tested examples
```

- `core/` holds providers, the quality framework, `dx_wrap`, and the toolchain and pins factory. It holds no language code.
- `config/` holds public build settings that are not specific to one language.
- `core/private/` holds core internals.
- `cli/` holds every Rust crate.
- `gazelle/` holds the shared Gazelle plugins.
- `tools/` holds scripts that only maintainers run, and the generators.
- `patches/` holds upstream patches. The patch table is generated from `MODULE.bazel`.
- `third_party/` holds vendored upstream sources.
- `docs/` holds user docs. Per-language pages are generated from `lang.json`.
- Every example in `examples/` is a tested target.
- The root holds only language directories with a `lang.json` and the directories above. A test enforces the list.

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
