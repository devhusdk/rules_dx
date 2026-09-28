# `dx check`, `dx fix`, And `dx clean`

```sh
bazel run //cli/cli:dx -- check //...
bazel run //cli/cli:dx -- fix //...
bazel run //cli/cli:dx -- clean
```

## `dx check` And `dx fix`

```text
dx check [--here] [--check] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...] [-- bazel-options...]
dx fix [--here] [--check] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...] [-- bazel-options...]
```

Runs `format`, then `lint`, then `typecheck`, then `generate` in order. Stops
on the first failure. `--fail-on` and `--report sarif=<dest>` pass through to
each phase. `dx check` is always a check, so `--check` is implied. Reports
merge the `lint` and `typecheck` phases; `-` for stdout is rejected because
the phases share one document.

Output: `--output text|diff|json`.

`dx check` only reports. `dx fix` applies fixes without re-running. Run
`dx check` again after `dx fix` to confirm.

Exit codes: `0` success, `2` usage or scope errors, `1` a phase failed.

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
