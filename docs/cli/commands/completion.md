# `dx completion`

```text
dx completion [<shell>] [--check]
```

Prints the completion script for one shell: `bash`, `zsh`, `fish`, or
`powershell`. Runs outside a workspace.

- With one shell and no `--check`: print that shell script.
- With `--check`: verify without writing. Zero shells checks all shells.
- Unknown shells fail with `unknown-shell`.

The script asks `dx` for candidates as you type, so keep `dx` on your `PATH`.
Candidates cover every command, every flag, the per-command slots, the
package labels of the selected workspace, and the targets of one package.

`--workspace` selects the workspace for candidates. Without it candidates
come from the current workspace. Nested workspaces and `.bazelignore`
entries never offer labels. Target completion queries one package only and
falls back to package patterns when the query cannot run. Completion never
writes files.

Output: `--output text`.

Exit codes: `0` success, `2` usage errors including an unknown shell, `1` a
completion check failed.

```sh
bazel run @rules_dx//:dx -- completion bash > ~/.cache/dx-completion.bash
bazel run @rules_dx//:dx -- completion --check
```