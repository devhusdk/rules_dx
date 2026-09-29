# `dx generate`

```sh
bazel run //cli/cli:dx -- generate //...
bazel run //cli/cli:dx -- generate --check //...
```

Refreshes Gazelle `BUILD` files over the scope. No scope means the whole
repo. Use `--here` for the current dir tree.

```text
dx generate [--here] [--check] [scope...] [-- bazel-options...]
```

`--check` fails if files are stale instead of writing them. Run
`dx generate` without `--check` to update, then `dx check` to confirm.

Exit codes: `0` success, `2` usage error, `1` stale or failed generation.
Bazel failures keep Bazel's code.
