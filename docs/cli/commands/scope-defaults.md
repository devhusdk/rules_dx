# Scope Defaults

No scope means `//...` for most commands: `build`, `test`, `coverage`,
`lint`, `typecheck`, `format`, `generate`, `security`, `license`, `check`,
and `fix`.

## Scope Shapes

- Labels and patterns: `//...`, `//pkg:target`, `@repo//...`.
- Files and dirs: workspace-relative paths resolved through `bazel query`
  to the owning targets.
- `--here` (`--cwd` alias): the current directory tree (`//path/...`,
  `//...` at the root). Never combines with explicit scopes.

## Commands That Need Args

- `dx run`, `dx deploy`, `dx why`, `dx bump`, `dx completion`: need their
  positional args. `dx deploy` takes exactly one label. `dx why` takes
  exactly one file plus one label.
- `dx bazel`: forwards raw args to Bazel. No scope resolution.
- `dx clean`, `dx status`, `dx version`: take no scopes.
- `dx env`, `dx codegen`, `dx setup`: repository-wide, or one exact `//`
  or `@` label.
