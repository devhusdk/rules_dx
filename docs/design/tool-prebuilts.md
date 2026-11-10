# Tool prebuilt design

This page fixes how dx supplies a quality tool on a host where upstream ships no usable binary. The artifact hub, the tool support rows and the release workflow cite it.

## Hosts

- The qualified hosts are `linux_x86_64`, `linux_arm64`, `macos_arm64` and `windows_x86_64`.
- `windows_arm64` resolves to the `windows_x86_64` artifact and runs it emulated. No tool gets a native `windows_arm64` build unless upstream ships one.
- A support row lists a host only when a matrix case ran on that host.
- A host that is not listed carries an `unsupported` reason, and the tool attribute resolves to `dx_unavailable_tool` there.

## Source order

Each tool and host takes the first rule that applies.

1. **Fetch upstream.** The upstream release carries a binary for the host, and the binary is static or its ABI floor is inside the hub floor.
2. **Consumer build.** The tool belongs to a language family whose rule set the consumer already loads, and the tool reads compiler output of the consumer's own toolchain.
3. **Release build.** The tool is a core default, upstream ships no usable binary for the host, and the locked source builds there with a hermetic toolchain.
4. **Unsupported.** The source does not build on the host, or the toolchain it needs has no build for the host.

## Fetch upstream

- The pin is the upstream URL plus the sha256 of the downloaded file and of the executable inside it.
- `checksum_source` names the upstream digest when upstream publishes one, and maintainer-established byte identity otherwise.
- A dynamically linked Linux binary records its ABI floor. A floor above the hub floor makes the binary unusable, and the host falls to the next rule.
- An upstream asset whose name carries no CPU is pinned for one host only, after a matrix case ran on that host.

## Consumer build

- The build uses the family's toolchain and a lock checked into the tool package.
- The tool target lives under the language directory, never under `core/`.
- A host is supported when the family toolchain has a cell for it and a `build_test` plus a matrix case pass there.
- These tools are never prebuilt.

## Release build

- One release job per qualified host builds the tool on a native runner from the lock in the tree, with the same hermetic toolchain the source target uses.
- The source target stays in the tree as the job input. A `build_test` covers it on each host the job builds. No core default reaches it.
- Linux artifacts are static.
- The asset name is `dx-tool-<tool>-<version>-<host>`. Its sha256 is listed in the attested checksum file of the release.
- The hub pins the asset like an upstream artifact. `checksum_source` reads `rules_dx release build`.
- The hub is the only place that chooses between an upstream artifact and a release-built one. Tool packages hold no `select()` over hosts.
- A core default is one alias into `@dx_tools`. Its package leaves `PENDING_TOOL_PACKAGES` in the same change.

## Licences

- Every redistributed asset ships the upstream licence text, and the artifact record names the licence and its source URL.
- MPL-2.0 and BSD-3-Clause tools are redistributed unmodified with the licence text.
- GPL-3.0 and LGPL-3.0 tools are redistributed only together with the exact source archive the binary was built from, published as an asset of the same release.
- A tool built with patches publishes the patches with that source archive.

## Hosting

Where release-built assets are hosted is the owner's decision. The rules above hold under either outcome.

- **rules_dx hosts the assets.** The release build rule applies as written.
- **rules_dx hosts no tool assets.** The release build rule does not exist. A core default that needs it keeps its source build and its `PENDING_TOOL_PACKAGES` entry, and a host whose source build fails is unsupported. Fetch upstream and consumer build are unaffected.

## Tools

`up` is fetch upstream, `rel` is release build, `own` is consumer build, `no` is unsupported.

| Tool | Licence | Upstream ships | linux_x86_64 | linux_arm64 | macos_arm64 | windows_x86_64 |
| --- | --- | --- | --- | --- | --- | --- |
| nixfmt | MPL-2.0 | one static Linux x86_64 binary | up | rel | rel | no |
| deadnix | GPL-3.0 | source only | rel | rel | rel | rel |
| selene | MPL-2.0 | Linux x86_64, macOS and Windows x86_64 zips | up | rel | up | up |
| zanuda | LGPL-3.0 | source only | own | own | own | no |
| weeder | BSD-3-Clause | source only | own | own | own | own |
| core Python tools | per wheel | wheels on PyPI | rel | rel | rel | rel |

- nixfmt: the executable depends on the `unix` package, so no Windows host is supported. `macos_arm64` is unsupported until the source target builds there.
- deadnix: the release carries the deadnix source archive next to each binary.
- selene: the Linux zip is dynamically linked. It is fetched only when its ABI floor is inside the hub floor, and is release-built otherwise.
- zanuda: it reads `.cmt` files of the consumer's OCaml. Windows is unsupported until the OCaml toolchain has a Windows cell.
- weeder: it reads `.hie` files and is built with the consumer's GHC. `windows_x86_64` is supported once its matrix case passes. `windows_arm64` is unsupported because GHC ships no bindist for it.
- Core Python tools: the release job assembles one bundle per host from a python-build-standalone interpreter and the wheels locked in `uv.lock`. Nothing is compiled. ansible-lint is unsupported on Windows.

## Source build inputs

| Tool | Toolchain | Locked packages |
| --- | --- | --- |
| nixfmt | GHC bindist, rules_haskell | 43 from Hackage |
| deadnix, selene | Rust toolchain, rules_rust | 240 crates in the shared lock |
| zanuda | OCaml compiler, dune | 35 in the duniverse |

- A consumer build that reaches one of these targets fetches the toolchain and every locked package.
- A release-built or fetched tool costs the consumer one download.
