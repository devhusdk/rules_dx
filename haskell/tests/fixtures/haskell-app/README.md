# Haskell App

A small stock tool that shows the Haskell rules with an external Hackage
dependency, a generated source, a C foreign call and a build flag.

- `stock` parses `name,quantity,price` lines with `Data.List.Split.splitOn`,
  imports a C checksum through `foreign import ccall`, and gets its version
  from a generated `Stock/Version.hs`.
- `cli` builds the `stockctl` binary, which reads stdin and exits 0 on valid
  input, 2 on a malformed line and 4 when a quantity exceeds the limit.

## Build, run and test

```sh
bazel build //haskell/tests/fixtures/haskell-app/...
printf 'lamp,4,1250\n' | bazel run //haskell/tests/fixtures/haskell-app/cli:stockctl
bazel test //haskell/tests/fixtures/haskell-app/...
```

## The limit flag

The string flag `//haskell/tests/fixtures/haskell-app/cli:limit` selects the quantity limit
compiled into `stockctl` through `-DSTOCK_LIMIT`. It defaults to `60`.

```sh
bazel test //haskell/tests/fixtures/haskell-app/... --//haskell/tests/fixtures/haskell-app/cli:limit=144
```

## The pinned dependency

`split` 0.2.5 comes from Hackage. `third_party/haskell_deps.json` pins its
hash and `.cabal` revision, and the `haskell.lock` tag in the root
`MODULE.bazel` turns that file into the `@haskell_app_hackage` hub, which
builds each package with `haskell_cabal_library`. Its license is
BSD-3-Clause. Refresh the pin with
`bazel run //haskell/tests/fixtures/haskell-app/third_party:resolve`.

`//haskell/tests/fixtures/haskell-app/cli:report_without_split` imports `Data.List.Split`
without declaring the dependency. It is tagged `manual`, and building it fails
with `Could not find module`.

The example is qualified on `linux_x86_64` only. Targets carry
`HASKELL_COMPATIBLE_WITH` and are skipped elsewhere.
