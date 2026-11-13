# Hackage executable fixture

`cabal.lock.json` pins `hspec-discover` 2.11.12, which has a library and an
executable component. The `cabal.lock` tag in the root `MODULE.bazel`
turns it into the `@haskell_exe_hackage` hub, and `hub_exe_build_test` builds
the library alias and the `hspec-discover-exe-hspec-discover` alias.
