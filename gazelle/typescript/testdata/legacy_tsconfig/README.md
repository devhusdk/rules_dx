# Legacy tsconfig Target

Verifies that a `ts_config(tsconfig)` loaded from the old
`@rules_dx//typescript:defs.bzl` export becomes `typescript_config(tsconfig)`
and that its dead load is replaced.
