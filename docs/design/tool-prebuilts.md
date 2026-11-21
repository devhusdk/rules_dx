# Tool prebuilt design

This page fixes how dx supplies a quality tool on a host where upstream ships no usable binary.

## Hosts

- The qualified hosts are `linux_x86_64`, `linux_arm64`, `macos_arm64` and `windows_x86_64`.
- `windows_arm64` resolves to the `windows_x86_64` artifact and runs it emulated. No tool gets a native `windows_arm64` build unless upstream ships one.
- A support row lists a host only when a matrix case ran there.
- An unlisted host carries an `unsupported` reason, and the tool attribute resolves to `dx_unavailable_tool` there.

## Source order

Each tool and host takes the first rule that applies.

1. **Fetch upstream.** The upstream release carries a binary for the host, and the binary is static or its ABI floor is inside the hub floor.
2. **Consumer build.** The tool belongs to a language family whose rule set the consumer loads and reads compiler output of the consumer's toolchain.
3. **Release build.** The tool is a core default, upstream ships no usable binary for the host, and the locked source builds there with a hermetic toolchain.
4. **Unsupported.** The source does not build on the host, or its toolchain has no build for it.

## Fetch upstream

- The pin is the upstream URL plus the sha256 of the download and of its executable.
- `checksum_source` names the upstream digest when upstream publishes one, and maintainer-established byte identity otherwise.
- A dynamically linked Linux binary records its ABI floor.
- A binary that fails the tool's smoke test is also unusable.
- An upstream asset whose name carries no CPU is pinned for one host only, after a matrix case ran there.

## Consumer build

- The build uses the family's toolchain and a lock checked into the tool package.
- The tool target lives under the language directory, never under `core/`.
- A host is supported when the family toolchain has a cell for it and a `build_test` plus a matrix case pass there.
- These tools are never prebuilt.

## Release build

- One release job per qualified host builds the tool on a native runner from the tree's lock with the source target's hermetic toolchain.
- The source target stays in the tree as the job input. A `build_test` covers it on each host the job builds. No core default reaches it.
- Linux artifacts are static.
- The asset name is `dx-<tool>-<os>-<cpu>`. The attested `dx-tool-sha256sums.txt` of the release lists its sha256.
- The registry `cli/prebuilt/tools.bzl` names the asset, and `@dx_prebuilt` serves the pinned download with the source target as its fallback.
- Only the hub chooses between an upstream artifact and a release-built one; tool packages hold no `select()` over hosts.
- A core default is one alias into `@dx_tools`. Its package leaves `PENDING_TOOL_PACKAGES` in the same change.

## Rule-set tools

A rule-set module (`rules_php`, `rules_lua`, `rules_qt`) builds its Rust tools with `@rules_dx//rust:defs.bzl`.

- Each rule-set release builds its Rust tools on `linux_x86_64`, `linux_arm64`, `macos_arm64`, `windows_x86_64` and `windows_arm64`.
- The asset name is `rules_<lang>-<tool>-<version>-<host>`. `rules_<lang>-sha256sums.txt` in the same release lists each asset's sha256.
- The rule-set extension pins each asset like `dx_prebuilt`, and a host with a pin fetches it.
- A host with no pin builds the tool from source with rules_dx's Rust and C toolchains. A root module that sets `from_source()` on the rule-set extension builds every host from source.
- A C or C++ tool that needs only the language's own C++ toolchain is built from source and never prebuilt.
- A fixture fetches each pinned tool on each host, and a `from_source()` fixture builds it.

Rule set | Tool | Source | Prebuilt
--- | --- | --- | ---
rules_php | PHP launcher | Rust | rules_dx `dx-launcher` through `native_launcher_repos`; no `rules_php-launcher` asset
rules_php | PCOV extension | C | yes
rules_php | composer lock resolver | Rust | yes
rules_lua | Lua launcher | C | no
rules_qt | Qt launcher | Rust | yes

## Licences

- Every redistributed asset ships the upstream licence text, and the artifact record names the licence and its source URL.
- When upstream ships no licence file, the repo ships a checked-in copy of the notice, and a test matches it against the pinned source.
- MPL-2.0 and BSD-3-Clause tools are redistributed unmodified.
- GPL-3.0 and LGPL-3.0 tools are redistributed only with the exact source archive the binary was built from, published as an asset of the same release.
- A tool built with patches publishes the patches with that source archive.
- Haskell tools link GMP statically: Linux from Alpine (`dx-haskell-gmp-LICENSE`, `dx-haskell-gmp-source.tar.xz`, `dx-haskell-gmp-source.patch`), macOS and Windows from GHC's in-tree copy (`dx-haskell-gmp-ghc-source.tar.xz`).

## Hosting

The owner decides where release-built assets are hosted.

- **rules_dx hosts the assets.** The release build rule applies as written. `release-tools.yml` builds them.
- **rules_dx hosts no tool assets.** The release build rule does not exist. A core default that needs it keeps its source build and its `PENDING_TOOL_PACKAGES` entry, and a host whose source build fails is unsupported.

## Tools

`up` is fetch upstream, `rel` is release build, `own` is consumer build, `no` is unsupported.

Tool | Licence | Upstream ships | linux_x86_64 | linux_arm64 | macos_arm64 | windows_x86_64
--- | --- | --- | --- | --- | --- | ---
nixfmt | MPL-2.0 | one static Linux x86_64 binary | up | rel | rel | no
deadnix | GPL-3.0 | source only | rel | rel | rel | rel
selene | MPL-2.0 | Linux x86_64, macOS and Windows x86_64 zips | rel | rel | rel | rel
kcov | GPL-2.0 | source only | rel | rel | no | no
zanuda | LGPL-3.0 | source only | own | own | own | no
weeder | BSD-3-Clause | source only | rel | rel | rel | rel
hlint | BSD-3-Clause | Linux x86_64 and Windows x86_64 binaries | up | rel | rel | up
ormolu | BSD-3-Clause | Linux x86_64, macOS arm64 and Windows x86_64 binaries | up | rel | up | up
core Python tools | per wheel | wheels on PyPI | rel | rel | rel | rel

- nixfmt: the executable depends on the `unix` package, so no Windows host is supported.
- deadnix: the release carries `dx-deadnix-source.crate`.
- selene: upstream zips lack the Lua 5.2 to 5.4 parsers. The default `std = "lua53+lua54"` (`lua/private/quality/selene.toml`) needs them, so every host is release-built from `@dx_selene_src`. Gate it on `//lua/tests:selene_smoke_test`.
- kcov: `//tools/coverage/kcov:kcov_static` builds it against musl, statically linking elfutils (with `patches/elfutils/musl.patch`) and zlib. `dx-kcov-source.tar.gz` holds the kcov tarball, its patches, `kcov.BUILD` and the elfutils and zlib sources. Gate it on `//tools/coverage/kcov:smoke_test` and `//tools/coverage/kcov:static_smoke_test`. elfutils, zlib and `@kcov` are rules_dx dev dependencies, so only the rules_dx root builds kcov from source. `DEV_SOURCE_FALLBACK` in `cli/prebuilt/tools.bzl` makes a consumer hub serve `@rules_dx//tools/coverage/kcov:none` on a platform with no kcov pin, and that test writes no coverage.
- zanuda reads `.cmt` files of the consumer's OCaml, whose toolchain has no Windows cell.
- weeder reads `.hie` files and is built with the consumer's GHC. `windows_arm64` is unsupported because GHC ships no bindist for it.
- hlint, ormolu and nixfmt: the `hackage` job in `release-tools.yml` builds `@haskell_<tool>_hackage` for each `rel` cell, against musl on linux arm64, and ships `dx-<tool>-LICENSE` from the locked source archive.
- Core Python tools: the `python-tools` job in `release-tools.yml` assembles one `dx-python-tools-<host>.tar.gz` bundle per host from a python-build-standalone interpreter and the wheels locked in `uv.lock`, compiling nothing. ansible-lint is unsupported on Windows.

## Source build inputs

Tool | Toolchain | Locked packages
--- | --- | ---
nixfmt | GHC bindist, rules_haskell | 43 from Hackage
deadnix, selene | Rust toolchain, rules_rust | 240 crates in the shared lock
kcov | C and C++ toolchain | elfutils and zlib from the BCR
zanuda | OCaml compiler, dune | 35 in the duniverse

- A source build fetches the toolchain and every locked package; a release-built or fetched tool is one download.
