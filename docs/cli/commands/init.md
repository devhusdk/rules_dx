# `dx init`

```sh
bazel run @rules_dx//:dx -- init --apply
```

## `dx init`

```text
dx init [--apply] [module-name]
```

Checks by default; `--apply` scaffolds. Scaffolds `dx` into a foreign tree. Takes an optional module name, which
defaults to `my_project`. The module name starts with `[a-z0-9]` and uses
`[a-z0-9._-]` only. An invalid module name fails before any file is written.
Never overwrites existing files. It writes `.dx/version`, `.envrc`, `.devcontainer/devcontainer.json`,
`.vscode/settings.json`, `.vscode/extensions.json`,
`.github/workflows/ci.yml` and `MODULE.bazel.snippet`. The CI caller pins the
`rules_dx` reusable workflow at a reviewed commit and passes
`rules_dx_version` matching the snippet. Editor settings point at the managed
interpreter and tools with host-native names.
Flags: `--apply`.
Scopes: optional single module name.

Output: `--output text`.

Exit codes: `0` success, `2` usage errors including extra positionals or an
invalid module name, `1` scaffolding failed.

## Git hooks

dx does not install or manage Git hooks. For a commit or push hook, write a
plain hook or use a hook tool, and have it run:

```sh
bazel run @rules_dx//:dx -- check //...
```

Enforce the same check in CI. See [GitHub CI](../../github-ci.md).
