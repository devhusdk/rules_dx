# Stack lock fixture

`stack.yaml.lock` names the `lts-24.15` snapshot and the `split-0.2.5`
extra-dep. `haskell_deps_resolve` with `stack_lock` turns it into
`cabal.project.freeze` and `haskell_deps.json`. The `haskell.lock` tag in the
root `MODULE.bazel` makes the `@haskell_stack_hackage` hub, which `stack_app`
uses for `safe`. `resolve_test` fails when either file is stale.
