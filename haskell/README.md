# Haskell

This directory helps you build, run and test Haskell code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//haskell:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [Haskell page](../docs/languages/haskell.md) shows
setup, every rule and the tools dx runs.

Run the Haskell tests from a rules_dx checkout:

```sh
tools/dx test //haskell/...
```

## Dependencies

### Lock Hackage dependencies

You need a directory with `.cabal` files and a `cabal.project` that sets
`index-state:`. Then:

1. Run `dx update --apply`. It runs the pinned cabal-install 3.18.2.0 and
   writes `cabal.project.freeze` and `cabal.lock.json`. This step needs
   network access to Hackage.
2. Run `dx generate --apply`. With `cabal.lock.json` beside the root
   `cabal.project`, it adds the `cabal.lock` stanza for the `@hackage` hub
   to `MODULE.bazel`. A hub is one Bazel repository that holds a target for
   every locked package.
3. Depend on `@hackage//:<package>` for a library, or on
   `@hackage//:<package>-exe-<name>` for an executable.

`dx update` without `--apply` checks the lock. `dx security`, `dx license`
and `dx update` read `cabal.project.freeze`.

`cabal.lock.json` lists each package with its version, sdist sha256,
`.cabal` revision and sha256, components and dependencies. A public
sublibrary of another package is recorded as `lib:<sub>`. Depend on it as
`<pkg>:<sub>`; the hub builds it as `<pkg>-lib-<sub>`.

rules_dx does not resolve `stack.yaml.lock`. `dx update` on a set whose only
lock is `stack.yaml.lock` tells you to resolve it first.

### How Hackage packages build

Each Hackage package becomes one repository. Bazel fetches its sdist by the
lock's sha256 and applies the pinned `.cabal` revision. Nothing is resolved
at fetch time. Libraries build with `haskell_cabal_library` and executables
with `haskell_cabal_binary`. Haddock is off for these packages.

## Quality

| Command | Tool | When it runs |
| --- | --- | --- |
| `dx format` | `ormolu` 0.9.0.0 | by default |
| `dx lint` | `hlint` 3.10 | by default |
| `dx typecheck` | `ghc -fno-code` | when a `haskell` family sets `typecheck = ["ghc"]` |

The [quality commands page](../docs/cli/commands/quality.md) covers the
flags and family setup.

## Coverage

Haskell tests write LCOV under `bazel coverage` and `dx coverage`. The test
and its libraries compile with `-fhpc`, and rules_dx turns the `.tix` and
`.mix` files that hpc writes into line coverage. You get line hits only, no
branch or function coverage.

## Bench and fuzz

`haskell_bench` builds a `haskell_binary` from
[tasty-bench](https://hackage.haskell.org/package/tasty-bench) benchmarks.
It adds a `dx-bench` test named `<name>` that runs the binary with `--csv`
and writes `dx-bench.json` for `dx bench`. Lock `tasty-bench` like any other
Hackage dependency. The [bench fixture](tests/fixtures/bench/BUILD.bazel) is
a working example.

There is no `haskell_fuzz_test`: Haskell has no maintained libFuzzer-style
fuzz harness.

## API docs

`dx docs` documents every `haskell_library` in scope. Nothing needs to be
installed.

- `haddock --hoogle` from the pinned GHC writes the declarations and their
  comments as Hoogle text.
- `haskell_doc_shard` from `@rules_dx//tools/docs/haskell:defs.bzl` turns
  that text into the API reference.
- Test and binary targets are skipped.
- A library that needs GHCi libraries, such as Template Haskell with C
  dependencies, fails in haddock with haddock's own error.

## Toolchain

### Runtime libraries

GHC and the programs it links load two shared libraries that are not part
of glibc. Both come from pinned Debian 10 `amd64` or `arm64` packages and
ship next to GHC, with their license texts in `dx_closure/licenses`:

| Library | Package | License |
| --- | --- | --- |
| `libgmp.so.10` | `libgmp10` 6.1.2 | LGPL-3.0-or-later OR GPL-2.0-or-later |
| `libtinfo.so.6` | `libtinfo6` 6.1 | X11 |

`haskell_binary` and `haskell_test` link GMP dynamically and carry
`libgmp.so.10` in their runfiles, the files Bazel places beside a program
when it runs. Ship `libgmp.so.10` with any binary you distribute.
`//haskell:licenses` has the license texts.

The host needs glibc 2.28 or newer.

### Host locale

GHC runs with `LANG=C.UTF-8` and loads that locale from the host. Check
that the host has it:

```sh
locale -a | grep -i c.utf
```

Without it, GHC falls back to ASCII and can fail on non-ASCII text with
`invalid byte sequence`. The toolchain ships no locale data, because glibc
can reject locale files built by another glibc version.

### Example

[haskell_app](tests/fixtures/haskell_app/README.md) is a multi-package app
with a pinned Hackage dependency, a generated module, a C foreign call and a
string flag.

## Platforms

The [support matrix](../docs/support-matrix.md) lists the platforms. GHC
9.10.3 has no Windows arm64 build.

On other platforms, Haskell targets fail toolchain resolution: Bazel finds
no Haskell toolchain for that platform.
