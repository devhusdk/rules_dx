# API spec example

An OpenAPI 3.1 root in `pets` that refers to a schema next to it and to an
error schema in `shared`. Redocly lints the root with every file its `$ref`
graph reaches.

| Package | Role |
| --- | --- |
| `pets` | `openapi.yaml` root and the `schemas/pet.yaml` ref |
| `shared` | `error.yaml`, a ref-only library other specs depend on |

Select Redocly for the `api_spec` family in your workspace policy:

```python
quality_family(
    name = "api_spec_family",
    family_id = "api_spec",
    lint = ["redocly"],
)
```

```sh
bazel build //examples/api-spec/...
bazel run @rules_dx//:dx -- lint //examples/api-spec/...
```

A clean spec exits `0`. A finding exits `1` and names the root file, line,
column, rule and message. A finding in a ref lands on line 1 of the root and
starts with the ref's own `file:line:col`.

For the rules, the config and the checks dx runs before Redocly, see
`docs/cli/commands/quality.md`.

| Platform | Status |
| --- | --- |
| Linux x86_64 | Supported |
| Linux arm64 | Not supported yet |
| macOS arm64 | Not supported yet |
| Windows x86_64 | Not supported yet |
