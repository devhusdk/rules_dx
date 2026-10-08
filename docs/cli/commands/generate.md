# `dx generate`

```sh
bazel run @rules_dx//:dx -- generate //...
bazel run @rules_dx//:dx -- generate --check //...
```

Refreshes Gazelle `BUILD` files over the scope. No scope means the whole
repo. Use `--here` for the current dir tree. Checks by default and reports
stale files without writing. Pass `--apply` to write them.

```text
dx generate [--here] [--check] [--apply] [scope...] [-- bazel-options...]
```

- `--check`: fail if files are stale instead of writing them. Same as the
  default.
- `--apply`: write generated sources. Without it files stay untouched.
- `--fail-on info|warning|error`: rejected. Generation reports staleness, not
  findings with severities.
- `--report`: rejected. No report format exists for this command.
- `--output text|diff|json`: result shape.

Exit codes: `0` success, `2` usage or scope errors, `1` stale or failed
generation. Bazel failures keep Bazel's code.
