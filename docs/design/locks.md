# Lock and hub design

This page fixes how languages lock, fetch and resolve third-party packages. Tool pins follow `layout.md`.

## Terms

- A dependency set is a directory with a root manifest of one ecosystem.
- A manifest listed by a workspace above it (`[workspace]` in `Cargo.toml`, `pnpm-workspace.yaml`, `[tool.uv.workspace]`, `workspace` in `pubspec.yaml`) roots no set. No other ecosystem has members.
- A lock is the one file a hub builds from.

## Registry

- Each ecosystem has one entry under `setup.hubs` in the `lang.json` of its first language below; the others name only it.
- The entry holds `ecosystem`, `manifest`, `lock`, `state`, `extension`, `tag`, `hub`, `resolver` and `imports`.
- `hub` is a repository name, or `*` for one repository per package.
- Code reads these values only through the generated registry.
- An ecosystem with no hub extension is not in `ecosystems`; a language with empty `ecosystems` has no set.

## Foreign formats

`dx security` and `dx license` read one version of each foreign format; any other fails and names the supported one.

Format | Supported version
--- | ---
`pnpm-lock.yaml` | `lockfileVersion` 9.0
`package-lock.json` | `lockfileVersion` 3
`.gitleaks.toml` | `[[allowlists]]`; `[allowlist]` is rejected

## Classes

An ecosystem is upstream class when all three hold, else dx class.

1. An upstream extension builds the hub from one lock file.
2. That file holds a digest for every archive.
3. Gazelle resolves imports from that file.

Upstream class, as ecosystem (languages): manifest; lock; hub.

- `cargo` (rust): `Cargo.toml`; `Cargo.lock`; `crates`.
- `uv` (python): `pyproject.toml`; `uv.lock`; `pypi`.
- `npm` (javascript, typescript, astro, mdx, svelte, vue): `package.json`; `pnpm-lock.yaml`; `npm`.
- `pub` (dart, flutter): `pubspec.yaml`; `pubspec.lock`; resolver `@rules_dx//dart/private/tools/lock`; `pub`.
- `go` (go): `go.mod`; `go.sum`; `*`.
- `maven` (java, kotlin, scala): `libs.versions.toml`; `maven_install.json`; resolver `@rules_dx//java/private/tools/lock`; `maven`.
- `julia` (julia): `Project.toml`; `Manifest.bazel.json`; `julia`.

dx class, as ecosystem (languages): manifest; state; hub. The lock is `<ecosystem>.lock.json`.

- `nuget` (csharp, fsharp, dotnet): `paket.dependencies`; `paket.lock`; `nuget`.
- `ruby` (ruby): `Gemfile`; `Gemfile.lock`; `gems`.
- `composer` (php): `composer.json`; `composer.lock`; `composer`.
- `luarocks` (lua): `*.rockspec`; none; `rocks`.
- `dub` (d): `dub.json`; `dub.lock.json`, state `dub.selections.json`; resolver `@rules_dx//d/private/tools/lock`; imports `provides`; `dub`.
- `zig` (zig): `build.zig.zon`; none; `zig`.
- `hex` (elixir, erlang): `mix.exs`, else `rebar.config`; `mix.lock`, else `rebar.lock`; `hex`.
- `cpan` (perl): `cpanfile`; `cpanfile.snapshot`; `cpan`.
- `cabal` (haskell): `cabal.project`; `cabal.project.freeze`; `hackage`.
- `opam` (ocaml): `*.opam`; `*.opam.locked`; `opam`.
- `swiftpm` (swift): `Package.swift`; `Package.resolved`; `swiftpkg`.
- `fpm` (fortran): `fpm.toml`; none; `fpm`.
- `powershell` (powershell): `PSGallery.requirements.psd1`; none; `psgallery`.
- `ansible` (ansible): `requirements.yml`; none; `ansible`.
- `bicep` (bicep): `*.bicep`; none; `bicep`.
- `terraform` (terraform): `*.tf`; `terraform.lock.json`, state `.terraform.lock.hcl`; resolver `@rules_dx//terraform/private/tools/lock`; imports `provides`; `dx_terraform_providers`.

## Manifest

- The manifest is in the ecosystem's own format, never `MODULE.bazel`. A user edits only it to change a dependency.

## Lock

- A dependency set has one lock, next to its root manifest.
- A resolver writes its own state file; no hub or Gazelle code reads it.
- Only the lock step writes a lock or state file. The tree holds no `*.digests.json`.
- A dx class lock names every closure package with one exact version and every archive with URLs and sha256.

## dx lock schema

- `core/modules/lock.schema.json` is the schema of each `<ecosystem>.lock.json`.
- The top keys are `schema`, `ecosystem`, `inputs` (the manifest and state file mapped to their sha256) and `packages`.
- `packages` is sorted by `name`. A package holds `name`, `version`, `archives`, `deps`, `provides`, and `dev` when development-only.
- An archive holds `urls`, `sha256`, and `strip_prefix` and `platform` when they apply.
- `provides` lists the package's import names.
- Other ecosystem data is under `extra`; the schema lists its keys per ecosystem.
- Keys are sorted, so the same inputs and registry state give the same bytes.

## Groups

- A `nuget.lock.json` holds one exact-version closure per Paket group under `groups`, which maps group name to packages; its `packages` is empty.
- Each group is its own hub: the `main` group is the set's hub name, another group is `<hub>.<group>`.
- A package of a group is `@<group hub>//<name>`.

## Extension

- A consumer loads a hub extension only from `@rules_dx//<lang>:extensions.bzl`, named for the ecosystem.
- A dx class extension has one tag, `lock`, with attributes `name` and `lock`. A development hub sets `dev_dependency = True`.
- An upstream class extension is the upstream one under the rules_dx label. Its tag takes the lock label, and the manifest label if upstream needs it.
- No tag has a `sha256`, `integrity`, `digest`, `digests`, `checksums`, `version` or `url` attribute.
- No package is declared with `use_repo_rule` or one tag per package.
- `lock_hub` in `core/pins.bzl` makes every dx class extension; the language passes one function returning a package's BUILD file.
- An archive downloads only with the lock's digest; a package without one fails the extension.
- Module extension facts hold only values derived from sha256-pinned inputs. An extension runs no resolver and returns `reproducible = True`.

## Hub

- The default hub name is the `hub` value. Another set is `<hub>_<dir>`, with `/` written as `.`.
- A set path with a character besides letters, digits, `_`, `-` and `/` fails generation.
- A `*` ecosystem has no hub name; `use_repo` lists the names its resolver prints with `--repos`.
- A dx class package is `@<hub>//<name>`, named as the ecosystem writes it. BUILD files never name a package repository.

## Stanza

- `dx init --apply` and `dx generate --apply` write the `use_extension`, tag and `use_repo` lines for each set whose lock no stanza names.
- The stanza names only `@rules_dx` labels and adds no `bazel_dep`.

## Lock step

- `resolver` labels one runnable rules_dx target. dx runs `bazel run <resolver> -- --dir <set directory>`, plus `--check` to compare only.
- A `dx_subcommand` resolver target runs `dx <lang>_lock` from the prebuilt dx, each `path_args` entry as `<flag>=<absolute path>`, so the step compiles nothing in a consumer.
- No user BUILD file declares a lock, pin, resolve or digest target.
- `dx generate --apply` runs the step for a set with a missing or stale lock, `dx update` to move versions, `dx check` with `--check`.
- A dx class lock is stale when an `inputs` file has another sha256. For upstream class the resolver's frozen mode decides.
- The `pub` and `maven` steps record the manifest's sha256 in the lock: `pubspec.lock` gets a `# dx inputs sha256: <hex>` line after pub's header comments, `maven_install.json` a top-level `"__DX_INPUTS_SHA256"` key. A matching marker is fresh without starting `dart` or `java`; a missing or different one is stale. Existing locks need one `dx generate --apply`.
- `dx check` and `dx generate` skip a `maven_install.json` with `__INPUT_ARTIFACTS_HASH`; rules_jvm_external pins it.
- The step runs the resolver rules_dx pins by sha256, starts no host program (see `host-tools.md`) and keeps the resolver's home, cache and config in a scratch directory.
- The `maven` resolver runs the rules_jvm_external Maven resolver with the pinned JDK. It reads `libs.versions.toml`, writes `maven_install.json`.
- The `pub` resolver runs `pub get` of the pinned Dart SDK on a copy of `pubspec.yaml` and `pubspec.lock`, with only the pinned coreutils `chmod` on `PATH`, and writes `pubspec.lock`.
- A step whose `inputs` match makes no network request, and a package with unchanged name, version and URLs keeps its entry.

## Imports

- Gazelle resolves a third-party import through the lock of the nearest set at or above it.
- `# gazelle:dx_external` only renames a hub or turns an ecosystem off.
- `imports` is `name`, where the ecosystem's fixed rule maps an import to a package name, or `provides`, where the longest matching entry wins.
- Every dx class ecosystem uses `provides`. One Gazelle parser reads every dx lock.
- An import of a locked package needs no `# gazelle:resolve` line. An import no lock provides fails generation and names the manifest to edit.

## Checks

- `//tools/ci:lock_registry_test` fails on an ecosystem without a registry entry, a banned tag attribute, a `*.digests.json` file, a Starlark lock parser, a manifest or lock name outside the registry, and a doc line pairing `gazelle:resolve` with a hub label. Its allowlist only shrinks.
- Each ecosystem has one fixture set under `<lang>/tests/fixtures/`. One test runs its resolver with an empty `PATH` against a registry fixture in runfiles and compares the lock bytes.
- One table test in `cli/adopt` plans the stanza for every registry entry.
- One table test in `gazelle/common` resolves an import from each ecosystem's fixture to its hub label with no directive.
