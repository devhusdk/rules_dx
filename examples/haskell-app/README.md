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
bazel build //examples/haskell-app/...
printf 'lamp,4,1250\n' | bazel run //examples/haskell-app/cli:stockctl
bazel test //examples/haskell-app/...
```

## The limit flag

The string flag `//examples/haskell-app/cli:limit` selects the quantity limit
compiled into `stockctl` through `-DSTOCK_LIMIT`. It defaults to `60`.

```sh
bazel test //examples/haskell-app/... --//examples/haskell-app/cli:limit=144
```

## The pinned dependency

`split` 0.2.5.1 comes from Hackage, pinned by hash in the root
`MODULE.bazel` as `haskell_app_split` and built by `split.BUILD.bazel`. Its
license is BSD-3-Clause. The overlay turns off the dx `-Werror` default for
this third-party code.

`//examples/haskell-app/cli:report_without_split` imports `Data.List.Split`
without declaring the dependency. It is tagged `manual`, and building it fails
with `Could not find module`.

The example is qualified on `linux_x86_64` only. Targets carry
`HASKELL_COMPATIBLE_WITH` and are skipped elsewhere.
