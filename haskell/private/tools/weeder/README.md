# weeder from Hackage

`cabal.project` and `weeder_tool.cabal` pin weeder 2.11.0 and its dependency
closure. `cabal.lock.json` is written by
`bazel run //haskell/private/tools/lock -- --dir haskell/private/tools/weeder`
and feeds the `@haskell_weeder_hackage` hub. `weeder_build_test` builds
`@haskell_weeder_hackage//:weeder-exe-weeder` with the managed GHC. The toolchain
hub exposes the `ghc` and `os-string` libraries for it.
