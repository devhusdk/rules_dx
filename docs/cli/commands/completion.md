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

Candidates come from the workspace a run would use: `--workspace`, then
`DX_WORKSPACE` (ignored in CI), then `workspace` in `dx.toml`, then the
current workspace. A broken `dx.toml` falls back to the current workspace
without output. Nested workspaces and `.bazelignore` entries never offer
labels.

Target completion runs one `bazel query` for the typed package, with the
typed `--bazel-startup-option` tokens before `query`. It gives up after about
1.5 seconds or when another Bazel command holds the server, and then offers
package patterns only. Completion never writes files and never prints
errors while you type.

Output: text only.

Exit codes: `0` success, `2` usage errors including a missing or unknown
shell, `1` stdout could not be written.

```sh
bazel run @rules_dx//:dx -- completion bash > ~/.cache/dx-completion.bash
```