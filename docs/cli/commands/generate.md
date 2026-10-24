# `dx generate`

```sh
bazel run @rules_dx//:dx -- generate //...
bazel run @rules_dx//:dx -- generate --check //...
```

Updates `BUILD` declarations from source with Gazelle over the scope. To
materialize declared generated source, use `dx codegen`. No scope means the whole
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

## Directives

`# gazelle:dx_ignore_import <language> <import>` skips one import in this
directory and below. `dx generate` fails when no literal reference matches it,
when the language is unknown, or when the value is malformed.

Accepted languages: `astro`, `cc`, `csharp`, `fsharp`, `gleam`, `go`, `java`,
`javascript`, `kotlin`, `mdx`, `python`, `ruby`, `rust`, `scala`, `svelte`,
`typescript`, `vue`, `zig`.

`# gazelle:resolve <language> <import> <label>` maps one import to an exact
label.

```starlark
# gazelle:dx_ignore_import python requests
# gazelle:resolve python yaml @pypi//pyyaml
```
