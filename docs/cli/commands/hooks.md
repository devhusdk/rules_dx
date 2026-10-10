# `dx init` And `dx hooks`

```sh
bazel run @rules_dx//:dx -- init --apply
bazel run @rules_dx//:dx -- hooks --apply install
```

## `dx init`

```text
dx init [--apply] [module-name]
```

Checks by default; `--apply` scaffolds. Scaffolds `dx` into a foreign tree. Takes an optional module name, which
defaults to `my_project`. The module name starts with `[a-z0-9]` and uses
`[a-z0-9._-]` only. An invalid module name fails before any file is written.
Never overwrites existing files.
Flags: `--apply`, `--dry-run`.
Scopes: optional single module name.

Output: `--output text`.

Exit codes: `0` success, `2` usage errors including extra positionals or an
invalid module name, `1` scaffolding failed.

## `dx hooks`

```text
dx hooks [--apply] <install|uninstall|status|run> [pre-commit|pre-push]
```

Manages Git hooks through hermetic Git. Checks by default; `--apply`
installs, removes, or records timings.

Install and uninstall ask the managed Git where hooks live, so linked
worktrees and `core.hooksPath` work. A relative `core.hooksPath` resolves
from the workspace. Reported hook paths are workspace-relative when inside
the workspace, otherwise absolute. Hooks without the dx marker are never
overwritten or removed; a foreign hook fails the command with exit `1`.

- `install`: install `pre-commit` and `pre-push` shims.
- `uninstall`: remove them.
- `status`: show what would run.
- `run <trigger>`: run one trigger.
Flags: `--apply`, `--dry-run`.
Scopes: verb `install|uninstall|status|run`.

Install, uninstall, and run need a hermetic Git. Set `DX_GIT_BIN`
to the absolute path of a managed Git binary. A relative path is rejected
and `PATH` is never searched. Without it the command exits `1` with
`hook git must be hermetic`. Outside a Git repository the command exits `1`.

Checks run read-only: a check without an explicit mode runs its command in
check mode, and only a check that names `--apply` applies. Measured timings
are written only when the run applies, either through `dx hooks --apply run`
or through a check that names `--apply`.

A check is command words. A plain string splits on whitespace with no shell.
A check that needs a space or quote inside one argument uses structured form.
A plain string that quotes an argument fails with an error that names the
structured spelling.

```toml
[hooks]
pre_commit = ["format --check", { command = "lint", args = ["--scope", "my dir"] }]
```

One budget covers the whole run. Each check receives the time that remains,
so a slow first check shortens the rest. A check that cannot start inside
the budget never launches. A hanging check and its descendants stop at the
deadline and the run reports a timeout. A check that prints past the capture
bound fails.

`run pre-commit` checks staged changes. `run pre-push` reads the pushed refs
from stdin and checks the outgoing commits, even with an empty index.
A deleted ref is skipped. A new remote ref diffs against the empty tree.

Change lists use NUL-delimited Git output, so spaces and quotes in names
survive. A non-UTF-8 name fails with an explicit error. A deleted path maps
to its nearest enclosing Bazel package, so it never vanishes silently.
Every run prints which source it selected. Checks read worktree files, not
staged bytes.

Output: `--output text`.

Exit codes: `0` success, `2` usage errors including an unknown verb or
trigger, `1` a hook install, status, discovery, or check failed. A failing
check reports `1`, never the check's own code.

```sh
bazel run @rules_dx//:dx -- hooks status
bazel run @rules_dx//:dx -- hooks run pre-commit
```
