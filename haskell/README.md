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
    deps = ["@dx_haskell_toolchains//:base"],
)

haskell_test(
    name = "stock_test",
    srcs = ["test/StockTest.hs"],
    deps = [
        ":stock",
        "@dx_haskell_toolchains//:base",
    ],
)

haskell_binary(
    name = "stockctl",
    srcs = ["src/Main.hs"],
    deps = [
        ":stock",
        "@dx_haskell_toolchains//:base",
    ],
)
```

| Rule | What it makes |
| --- | --- |
| `haskell_library` | Haskell package from `srcs` |
| `haskell_binary` | Executable with a `Main` module, run with `bazel run` |
| `haskell_test` | Test executable; a non-zero exit fails the test |

- A warning fails the build: `ghcopts` defaults to `["-Wall", "-Werror"]`.
  Set `ghcopts` to replace the default.
- Library `tags` reach the library; the generated upstream target is
  `manual`, so `//...` builds only the library.
- `@dx_haskell_toolchains//:<package>` has the GHC boot packages: `array`,
  `base`, `binary`, `bytestring`, `containers`, `deepseq`, `directory`,
  `exceptions`, `filepath`, `ghc-prim`, `mtl`, `parsec`, `pretty`, `process`,
  `stm`, `template-haskell`, `text`, `time`, `transformers` and `unix`.
- A `cc_library` in `deps` is callable through `foreign import ccall`.
- C compiling, linking and object merging run on the rules_dx C++ toolchain.
  GHC reads no compiler, linker or preprocessor from the host.

## Runtime libraries

GHC and the programs it links load two shared libraries that are not in
glibc. Both come from pinned Debian 10 packages and ship next to GHC with
their license texts in `dx_closure/licenses`:

| Library | Package | License |
| --- | --- | --- |
| `libgmp.so.10` | `libgmp10` 6.1.2 | LGPL-3.0-or-later OR GPL-2.0-or-later |
| `libtinfo.so.6` | `libtinfo6` 6.1 | X11 |

`haskell_binary` and `haskell_test` link GMP dynamically and carry
`libgmp.so.10` in their runfiles. Ship it with a binary you distribute.
`@dx_haskell_toolchains//:licenses` has the license texts.

The host needs glibc 2.28 or newer.

## Host locale

GHC runs with `LANG=C.UTF-8` and loads that locale from the host. The
toolchain ships no locale data, because glibc can reject locale files built
by another glibc version. If the host has no `C.UTF-8` locale, GHC falls back
to ASCII and can fail on non-ASCII text with `invalid byte sequence`. Check
with `locale -a | grep -i c.utf`.

## Formatting

No Haskell formatter or linter runs yet.

## Platforms

| Cell | Status |
| --- | --- |
| `linux_x86_64` | Qualified |
| `linux_arm64` | Not qualified |
| `macos_x86_64` | Not qualified |
| `macos_arm64` | Not qualified |
| `windows_x86_64` | Not qualified |

On other platforms Haskell targets fail toolchain resolution.

## Example

See [examples/haskell-app](../examples/haskell-app/README.md) for a multi-package app with a pinned Hackage dependency, a generated module, a C foreign call and a string flag.
