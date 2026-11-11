# Lock and hub design

This page fixes how every language locks, fetches and resolves third-party packages. Tool pins follow `layout.md`.

## Terms

- A dependency set is a directory with a root manifest of one ecosystem.
- A manifest listed by a workspace above it roots no set: `[workspace]` in `Cargo.toml`, `pnpm-workspace.yaml`, `[tool.uv.workspace]` or `workspace` in `pubspec.yaml`. No other ecosystem has members.
- A lock is the one file a hub builds from.

## Registry

- Each ecosystem has one entry under `setup.hubs` in the `lang.json` of its first language below; the others name only the ecosystem.
- The entry holds `ecosystem`, `manifest`, `lock`, `state`, `extension`, `tag`, `hub`, `resolver` and `imports`.
- `hub` is a repository name, or `*` for one repository per package.
- Code reads these values through the generated registry. No other source file states a manifest name, lock name, hub name or extension label.
- `lockfiles` lists only locks of this registry.
- An ecosystem with no hub extension is not listed in `ecosystems`. A language with an empty `ecosystems` has no set.

## Classes

An ecosystem is in the upstream class when all three hold, else the dx class.

1. An upstream extension builds the hub from one lock file.
2. That file holds a digest for every archive.
3. Gazelle resolves imports from that file.

Upstream class, as ecosystem (languages): manifest; lock; hub.

- `cargo` (rust): `Cargo.toml`; `Cargo.lock`; `crates`.
- `uv` (python): `pyproject.toml`; `uv.lock`; `pypi`.
- `npm` (javascript, typescript, astro, mdx, svelte, vue): `package.json`; `pnpm-lock.yaml`; `npm`.
- `pub` (dart, flutter): `pubspec.yaml`; `pubspec.lock`; `pub`.
- `go` (go): `go.mod`; `go.sum`; `*`.
- `maven` (java, kotlin, scala): `libs.versions.toml`; `maven_install.json`; `maven`.
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
- `terraform` (terraform): `*.tf`; `.terraform.lock.hcl`; `terraform`.

## Manifest

- The manifest is in the ecosystem's own format. `MODULE.bazel` is never a manifest.
- A user changes a dependency by editing only the manifest.

## Lock

- A dependency set has exactly one lock, next to its root manifest.
- A resolver writes a state file for itself. No hub or Gazelle code reads it.
- Only the lock step writes a lock or state file.
- The tree holds no `*.digests.json`.
- A dx class lock names every package of the closure with one exact version, and every archive with its URLs and a sha256.

## dx lock schema

- `core/modules/lock.schema.json` is the schema of every `<ecosystem>.lock.json`.
- The top keys are `schema`, `ecosystem`, `inputs` and `packages`.
- `inputs` maps the manifest and state file to their sha256.
- `packages` is sorted by `name`. A package holds `name`, `version`, `archives`, `deps`, `provides`, and `dev` when development-only.
- An archive holds `urls`, `sha256`, and `strip_prefix` and `platform` when they apply.
- `provides` lists the package's import names.
- Other ecosystem data is under `extra`. The schema lists its keys per ecosystem.
- Keys are sorted. The same inputs and registry state give the same bytes.

## Extension

- A consumer loads a hub extension only from `@rules_dx//<lang>:extensions.bzl`. Its name is the ecosystem.
- A dx class extension has one tag, `lock`, with attributes `name` and `lock`. A development hub sets `dev_dependency = True` on `use_extension`.
- An upstream class extension is the upstream one under the rules_dx label. Its tag takes the lock label, and the manifest label when upstream needs it.
- No tag has a `sha256`, `integrity`, `digest`, `digests`, `checksums`, `version` or `url` attribute.
- No package is declared with `use_repo_rule` or one tag per package.
- `lock_hub` in `core/pins.bzl` makes every dx class extension. The language passes one function returning a package's BUILD file.
- `lock_hub` reads the lock with `json.decode`. No Starlark code parses another lock format.
- An archive is downloaded only with the lock's digest. A package without one fails the extension. Nothing downloads first and prints a digest.
- Module extension facts hold only values derived from sha256-pinned inputs.
- An extension runs no resolver and returns `reproducible = True`.

## Hub

- The default hub name is the `hub` value. Another set of the ecosystem is `<hub>_<dir>`, with `/` written as `.`.
- A set path with a character other than letters, digits, `_`, `-` and `/` fails generation.
- A `*` ecosystem has no hub name. `use_repo` lists the names its resolver prints with `--repos`.
- A dx class package is `@<hub>//<name>`, named as the ecosystem writes it.
- BUILD files never name a package repository.

## Stanza

- `dx init --apply` and `dx generate --apply` write the `use_extension`, tag and `use_repo` lines for every set whose lock no stanza names.
- One planner loop over the registry writes them. No planner code names an ecosystem.
- The stanza names only `@rules_dx` labels and adds no `bazel_dep`.
- A second run plans nothing.
- No doc asks a user to type a stanza.

## Lock step

- `resolver` labels one runnable rules_dx target. dx runs `bazel run <resolver> -- --dir <set directory>`, with `--check` to compare only.
- No user BUILD file declares a lock, pin, resolve or digest target.
- `dx generate --apply` runs the step for a set with a missing or stale lock. `dx update` runs it to move versions. `dx check` runs it with `--check`.
- A dx class lock is stale when a file in `inputs` has another sha256. For the upstream class the resolver's frozen mode decides.
- The step runs the resolver rules_dx pins by sha256. It starts no host program and follows `host-tools.md`.
- The resolver's home, cache and config are in a scratch directory.
- `//cli/lock` is the only writer of `<ecosystem>.lock.json`.
- A step whose `inputs` match makes no network request.
- A package with unchanged name, version and URLs keeps its entry with no download.

## Imports

- Gazelle resolves a third-party import through the lock of the nearest set at or above the directory.
- `# gazelle:dx_external` only renames a hub or turns an ecosystem off.
- `imports` is `name` or `provides`. With `name`, the ecosystem's fixed rule maps the import to a package name. With `provides`, the longest matching entry wins.
- Every dx class ecosystem uses `provides`. One Gazelle parser reads every dx lock.
- An import of a locked package needs no `# gazelle:resolve` line.
- An import no lock provides fails generation and names the manifest to edit.

## rules_dx itself

- rules_dx locks its own sets with the same registry, lock step and stanza writer. Its `MODULE.bazel` holds no registry package digest.

## Checks

- `//tools/ci:lock_registry_test` fails on an ecosystem without a registry entry, a banned tag attribute, a `*.digests.json` file, a Starlark lock parser, a manifest or lock name outside the registry, and a doc line pairing `gazelle:resolve` with a hub label. Its allowlist only shrinks.
- Each ecosystem has one fixture set under `<lang>/tests/fixtures/`. One test starts its resolver with an empty `PATH` against a registry fixture in runfiles and compares the lock bytes.
- One table test in `cli/adopt` plans the stanza for every registry entry.
- One table test in `gazelle/common` resolves an import from every ecosystem's fixture to its hub label with no directive.
