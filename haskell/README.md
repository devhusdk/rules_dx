# Haskell

Build, run and test Haskell with a pinned GHC 9.10.3. Nothing is installed on
the host and nothing is fetched until a target needs it. rules_dx registers
the toolchain, so there is no setup in `MODULE.bazel`.

## Dependencies

### Resolving Hackage dependencies

The toolchain pins cabal-install 3.18.2.0 next to GHC.
`bazel run @rules_dx//haskell/private/tools/lock -- --dir <set directory>`
runs it for a directory with `.cabal` files and a `cabal.project` that sets `index-state:`. It writes `cabal.project.freeze` and `cabal.lock.json`, which lists each package with its version, sdist sha256, `.cabal` revision and sha256, components and dependencies. A public sublibrary of another package is recorded as `lib:<sub>` and depended on as `<pkg>:<sub>`, which the hub builds as `<pkg>-lib-<sub>`. Resolving needs network access to Hackage. `dx update` and `dx update --check` run the same step. `dx security`, `dx license` and `dx update` read `cabal.project.freeze`. `stack.yaml.lock` is not resolved; `dx update` of a set whose only lock is `stack.yaml.lock` reports that the lock must be resolved first.

### Building Hackage packages

With `cabal.lock.json` beside the root `cabal.project`, `dx generate --apply` adds the `cabal.lock` stanza for the `@hackage` hub to `MODULE.bazel`.

Each Hackage package becomes one repository fetched from its sdist with the lock sha256 and the pinned `.cabal` revision, built with `haskell_cabal_library` and `haskell_cabal_binary`. Nothing is resolved at fetch time. The hub has `@hackage//:<package>` for libraries and `@hackage//:<package>-exe-<name>` for executables. Haddock is off for these packages.

## Quality

`dx format` runs `ormolu` 0.9.0.0 on Haskell sources by default. See the Haskell section of `docs/cli/commands/quality.md`. `dx typecheck` runs `ghc -fno-code` when a `haskell` family opts in with `typecheck = ["ghc"]`. `dx lint` runs `hlint` 3.10 by default.

## Coverage

Haskell tests write LCOV under `bazel coverage` and `dx coverage`. rules_haskell compiles the test and its libraries with `-fhpc`, and rules_dx converts the `.tix` and `.mix` files hpc writes into line coverage. Only the lines of each expression are reported, not branches or functions.

## Bench and fuzz

`haskell_bench` builds a `haskell_binary` from [tasty-bench](https://hackage.haskell.org/package/tasty-bench) benchmarks and adds a `dx-bench` test `<name>` that runs it with `--csv` and writes `dx-bench.json` for `dx bench`. Resolve `tasty-bench` through `cabal.lock` like any Hackage dependency, see [bench](tests/fixtures/bench/BUILD.bazel).

There is no `haskell_fuzz_test`: Haskell has no maintained libFuzzer-style fuzz harness.

## API docs

`dx docs` documents every `haskell_library` in scope with the `haddock` of the pinned GHC bindist: `haddock --hoogle` writes the declarations and their comments as Hoogle text, and `haskell_doc_shard` (`@rules_dx//tools/docs/haskell:defs.bzl`) turns them into the API reference. Nothing needs to be installed. Test and binary targets are skipped. A library that needs GHCi libraries (Template Haskell with C dependencies) fails in haddock with its own error.

## Toolchain

### Runtime libraries

GHC and the programs it links load two shared libraries that are not in
glibc. Both come from pinned Debian 10 `amd64` or `arm64` packages, one set
per cell, and ship next to GHC with
their license texts in `dx_closure/licenses`:

| Library | Package | License |
| --- | --- | --- |
| `libgmp.so.10` | `libgmp10` 6.1.2 | LGPL-3.0-or-later OR GPL-2.0-or-later |
| `libgmp.a` | `libgmp-dev` 6.1.2 | LGPL-3.0-or-later OR GPL-2.0-or-later |
| `libtinfo.so.6` | `libtinfo6` 6.1 | X11 |

`haskell_binary` and `haskell_test` link GMP dynamically and carry
`libgmp.so.10` in their runfiles. Ship it with a binary you distribute.
`//haskell:licenses` has the license texts.

The host needs glibc 2.28 or newer.

### Host locale

GHC runs with `LANG=C.UTF-8` and loads that locale from the host. The
toolchain ships no locale data, because glibc can reject locale files built
by another glibc version. If the host has no `C.UTF-8` locale, GHC falls back
to ASCII and can fail on non-ASCII text with `invalid byte sequence`. Check
with `locale -a | grep -i c.utf`.

### Example

See [haskell_app](tests/fixtures/haskell_app/README.md) for a multi-package app with a pinned Hackage dependency, a generated module, a C foreign call and a string flag.

## Platforms

See the [support matrix](../docs/support-matrix.md).

GHC 9.10.3 ships no Windows arm64 bindist.

On other platforms Haskell targets fail toolchain resolution.
