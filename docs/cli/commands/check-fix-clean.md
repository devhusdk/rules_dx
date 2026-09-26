# `dx check`, `dx fix`, And `dx clean`

```sh
bazel run //cli/cli:dx -- check //...
bazel run //cli/cli:dx -- fix //...
bazel run //cli/cli:dx -- clean
```

## `dx check` And `dx fix`

```text
dx check [--here] [scope...] [-- bazel-options...]
dx fix [--here] [scope...] [-- bazel-options...]
```

Runs `format`, then `lint`, then `typecheck`, then `generate` in order. Stops
on the first failure. `--check`, `--fail-on`, and `--report` pass through to
each phase.

`dx check` only reports. `dx fix` applies fixes without re-running. Run
`dx check` again after `dx fix` to confirm.

```sh
bazel run //cli/cli:dx -- check //...
bazel run //cli/cli:dx -- fix --here
```

## `dx clean`

```text
dx clean [--dry-run] [--bazel]
```

Prunes unselected managed state under `.dx`. Never touches Bazel outputs
unless `--bazel` also runs `bazel clean`. Takes no scopes.
`--dry-run` only lists what would go.

```sh
bazel run //cli/cli:dx -- clean --dry-run
```
