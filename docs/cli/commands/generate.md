# `dx generate`

```sh
bazel run @rules_dx//:dx -- generate //...
bazel run @rules_dx//:dx -- generate --check //...
```

Refreshes Gazelle `BUILD` files over the scope. No scope means the whole
repo. Use `--here` for the current dir tree.

```text
dx generate [--here] [--check] [--apply] [scope...] [-- bazel-options...]
```

- `--check`: fail if files are stale instead of writing them. Run
  `dx generate` without `--check` to update, then `dx check` to confirm.
- `--fail-on info|warning|error`: rejected. Generation reports staleness, not
  findings with severities.
- `--report`: rejected. No report format exists for this command.
- `--output text|diff|json`: result shape.

Exit codes: `0` success, `2` usage or scope errors, `1` stale or failed
generation. Bazel failures keep Bazel's code.
