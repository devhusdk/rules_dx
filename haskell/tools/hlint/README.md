# hlint from Hackage

`cabal.project` and `hlint_tool.cabal` pin hlint 3.10 and its dependency
closure, with `ghc-lib-parser` 9.12 as hlint requires on GHC 9.10. `haskell_deps.json`
is written by `bazel run //haskell/tools/hlint:resolve` and feeds the
`@haskell_hlint_hackage` hub. The `hlint` alias selects this build on
`linux_arm64` and `macos_arm64`, and the upstream binary elsewhere.

`attoparsec` ships a public `attoparsec-internal` sublibrary. The resolver
keeps sublibraries in `components` and lists in-package edges in
`component_dependencies`; the hub builds each as `<package>-lib-<sublibrary>`.
`alex` and `happy` build as tools of `ghc-lib-parser`, which needs `python3`
from the rules_python interpreter on `PATH`.
