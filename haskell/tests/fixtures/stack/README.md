# Stack lock fixture

`stack.yaml.lock` names the `lts-24.15` snapshot and the `split-0.2.5`
extra-dep. The cabal lock step writes `cabal.project.freeze` and
`cabal.lock.json` from `cabal.project`. The `cabal.lock` tag in the
root `MODULE.bazel` makes the `@haskell_stack_hackage` hub, which `stack_app`
uses for `safe`.
