# `dx init`

```sh
bazel run @rules_dx//:dx -- init --apply
```

## `dx init`

```text
dx init [--apply] [module-name]
```

Adopts `dx` into an existing tree. Use `dx new` to start a new project. Checks by default; `--apply` scaffolds. Takes an optional module name, which
defaults to `my_project`. The module name starts with `[a-z0-9]` and uses
`[a-z0-9._-]` only. An invalid module name fails before any file is written.
Never overwrites existing files. It writes `.dx/version`, `.gitignore`, `.envrc`, `.devcontainer/devcontainer.json`,
`.vscode/settings.json`, `.vscode/extensions.json`,
`.github/workflows/ci.yml`, `MODULE.bazel.snippet`, and `.claude/settings.json`
with the `dx worktree` hooks. It also writes a marked block to `AGENTS.md`,
creating the file when it is missing (see below). If `.claude/settings.json`
already exists, it is left alone and the missing hook entries are printed to
stderr. The CI caller pins the
`rules_dx` reusable workflow at a reviewed commit and passes
`rules_dx_version` matching the snippet. The `.gitignore` lists `target/`, `/bazel-*`, `node_modules/`,
`__pycache__/`, `*.pyc`, `.gradle/` and `/build/`. Editor settings point at the managed
interpreter and tools with host-native names.
Flags: `--apply`.
Scopes: optional single module name.

Output: text only.

Exit codes: `0` success, `2` usage errors including extra positionals or an
invalid module name, `1` scaffolding failed.

## Agent commands block

`dx init --apply` writes this block between `<!-- dx:begin -->` and
`<!-- dx:end -->` markers in `AGENTS.md`:

```text
<!-- dx:begin -->
- Start: `tools/dx worktree add <branch>`. Finish: `tools/dx worktree release`.
- Fix first: `tools/dx fix --affected`.
- Then verify: `tools/dx check --affected`.
- Read the `--output=json` result.
<!-- dx:end -->
```

An existing file gets the block appended. A block that is out of date is
replaced. Text outside the markers is never touched. `dx upgrade --apply`
refreshes an existing block and never creates the file or adds a block.
`dx status` reports the `agents` check as `warn` when the block is missing or
out of date.

## Git hooks

dx does not install or manage Git hooks. For a commit or push hook, write a
plain hook or use a hook tool, and have it run:

```sh
bazel run @rules_dx//:dx -- check //...
```

Enforce the same check in CI. See [GitHub CI](../../github-ci.md).
