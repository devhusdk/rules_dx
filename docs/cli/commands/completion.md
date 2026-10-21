# `dx completion`

```text
dx completion <shell>
```

Prints the completion script for one shell: `bash`, `zsh`, `fish`, or
`powershell`. Runs outside a workspace.

- Takes exactly one shell and prints its script to stdout.
- No shell, more than one shell, or `--check` is a usage error.
- Unknown shells fail with `unknown-shell`.

The script asks `dx` for candidates as you type, so keep `dx` on your `PATH`.
Candidates cover every command, every flag, the per-command slots, the
package labels of the selected workspace, and the targets of one package.

`--workspace` selects the workspace for candidates. Without it candidates
come from the current workspace. Nested workspaces and `.bazelignore`
entries never offer labels. Target completion queries one package only and
falls back to package patterns when the query cannot run. Completion never
writes files.

Output: text only.

Exit codes: `0` success, `2` usage errors including a missing or unknown
shell, `1` stdout could not be written.

```sh
bazel run @rules_dx//:dx -- completion bash > ~/.cache/dx-completion.bash
```