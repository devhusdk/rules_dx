# Haskell

Build, run and test Haskell with a pinned GHC 9.10.3. Nothing is installed on
the host and nothing is fetched until a target needs it. rules_dx registers
the toolchain, so there is no setup in `MODULE.bazel`.

## Rules

Load the rules from `@rules_dx//haskell/rules:defs.bzl`. They wrap
[rules_haskell](https://github.com/tweag/rules_haskell) and take the same
attributes.

```python
load("@rules_dx//haskell/rules:defs.bzl", "haskell_binary", "haskell_library", "haskell_test")

haskell_library(
    name = "stock",
    srcs = ["src/Stock.hs"],
    deps = ["//haskell/toolchain:base"],
)

haskell_test(
    name = "stock_test",
    srcs = ["test/StockTest.hs"],
    deps = [
        ":stock",
        "//haskell/toolchain:base",
    ],
)

haskell_binary(
    name = "stockctl",
    srcs = ["src/Main.hs"],
    deps = [
        ":stock",
        "//haskell/toolchain:base",
    ],
)
```

| Rule | What it makes |
| --- | --- |
| `haskell_library` | Haskell package from `srcs` |
| `haskell_binary` | Executable with a `Main` module, run with `bazel run` |
| `haskell_test` | Test executable; a non-zero exit fails the test |

- A warning fails the build: `-Werror` is added to `ghcopts`, which defaults to
  `["-Wall"]`. Set `warnings_as_errors = False` on a target, or
  `--@rules_dx//config:warnings_as_errors=false`, to turn it off.
- Library `tags` reach the library; the generated upstream target is
  `manual`, so `//...` builds only the library.
- `//haskell/toolchain:<package>` has the GHC boot packages: `array`,
  `base`, `binary`, `bytestring`, `containers`, `deepseq`, `directory`,
  `exceptions`, `filepath`, `ghc-prim`, `mtl`, `parsec`, `pretty`, `process`,
  `stm`, `template-haskell`, `text`, `time`, `transformers` and `unix`.
- A `cc_library` in `deps` is callable through `foreign import ccall`.
- C compiling, linking and object merging run on the rules_dx C++ toolchain.
  GHC reads no compiler, linker or preprocessor from the host.

## Runtime libraries

GHC and the programs it links load two shared libraries that are not in
glibc. Both come from pinned Debian 10 `amd64` or `arm64` packages, one set
per cell, and ship next to GHC with
their license texts in `dx_closure/licenses`:

| Library | Package | License |
| --- | --- | --- |
| `libgmp.so.10` | `libgmp10` 6.1.2 | LGPL-3.0-or-later OR GPL-2.0-or-later |
| `libtinfo.so.6` | `libtinfo6` 6.1 | X11 |

`haskell_binary` and `haskell_test` link GMP dynamically and carry
`libgmp.so.10` in their runfiles. Ship it with a binary you distribute.
`//haskell/toolchain:licenses` has the license texts.

The host needs glibc 2.28 or newer.

## Host locale

GHC runs with `LANG=C.UTF-8` and loads that locale from the host. The
toolchain ships no locale data, because glibc can reject locale files built
by another glibc version. If the host has no `C.UTF-8` locale, GHC falls back
to ASCII and can fail on non-ASCII text with `invalid byte sequence`. Check
with `locale -a | grep -i c.utf`.

## Formatting

`dx format` runs `ormolu` 0.9.0.0 on Haskell sources by default, on Linux x86_64. See the Haskell section of `docs/cli/commands/quality.md`. `dx typecheck` runs `ghc -fno-code` when a `haskell` family opts in with `typecheck = ["ghc"]`. `dx lint` runs `hlint` 3.10 by default, on Linux x86_64 only; other platforms skip it with a note.

## Coverage

Haskell tests write LCOV under `bazel coverage` and `dx coverage`. rules_haskell compiles the test and its libraries with `-fhpc`, and rules_dx converts the `.tix` and `.mix` files hpc writes into line coverage. Only the lines of each expression are reported, not branches or functions.

## Platforms

| Cell | Status | Reason |
| --- | --- | --- |
| `linux_x86_64` | Qualified | |
| `linux_arm64` | Qualified | |
| `macos_arm64` | Qualified | CI job `checks (macos_arm64)`. |
| `macos_x86_64` | Not qualified | No CI cell. |
| `windows_x86_64` | Qualified | CI job `checks (windows_x86_64)`. |
| `windows_arm64` | n/a | GHC 9.10.3 ships no Windows arm64 bindist. |

On other platforms Haskell targets fail toolchain resolution.

## Resolving Hackage dependencies

The toolchain pins cabal-install 3.18.2.0 next to GHC. `haskell_deps_resolve` (`bazel run`) runs it for a directory with a `.cabal` file and a `cabal.project` that sets `index-state:`. It writes `cabal.project.freeze` and `haskell_deps.json`, which lists each package with its version, sdist sha256, `.cabal` revision and sha256, components and dependencies. Resolving needs network access to Hackage. `haskell_deps_resolve_test` reads the committed files only and fails when either is stale.

```starlark
load("@rules_dx//haskell/rules:deps_resolve.bzl", "haskell_deps_resolve", "haskell_deps_resolve_test")

haskell_deps_resolve(name = "resolve", srcs = ["cabal.project", "app.cabal"])

haskell_deps_resolve_test(name = "resolve_test", srcs = ["cabal.project", "app.cabal"])
```

The resolve runs on Linux and macOS. It uses the host `cc` where the pinned GHC names none.

## Building Hackage packages

The `haskell.packages` tag turns a `haskell_deps.json` into a hub repository:

```starlark
haskell = use_extension("@rules_dx//haskell/toolchain:repos.bzl", "haskell")
haskell.packages(name = "hackage", deps = "//third_party:haskell_deps.json")
use_repo(haskell, "hackage")
```

Each Hackage package becomes one repository fetched from its sdist with the sidecar sha256 and the pinned `.cabal` revision, built with `haskell_cabal_library` and `haskell_cabal_binary`. Nothing is resolved at fetch time. The hub has `@hackage//:<package>` for libraries and `@hackage//:<package>-exe-<name>` for executables. Haddock is off for these packages.

## Example

See [haskell-app](tests/fixtures/haskell-app/README.md) for a multi-package app with a pinned Hackage dependency, a generated module, a C foreign call and a string flag.
