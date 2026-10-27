# `dx completion`

```text
dx completion <shell>
```

Prints the completion script for one shell: `bash`, `zsh`, `fish`, or
`powershell`. Runs outside a workspace.

- Takes exactly one shell and prints its script to stdout.
- No shell, more than one shell, `--check`, `--output=json`,
  `--output=diff`, or `--` Bazel options is a usage error.
- Unknown shells fail with `unknown-shell`.

The script asks `dx` for candidates as you type, so keep `dx` on your `PATH`.
Candidates cover every command, every flag, the per-command slots, the
package labels of the selected workspace, and the targets of one package.

Candidates come from the workspace a run would use: `--workspace`, then
`DX_WORKSPACE` (ignored in CI), then `workspace` in `dx.toml`, then the
current workspace. While you type, a broken `dx.toml` falls back to the
current workspace without output. `dx completion <shell>` itself fails with
`invalid_defaults` when `dx.toml` is invalid. Nested workspaces and
`.bazelignore` entries never offer labels.

Target completion runs one `bazel query` for the typed package, with the
typed `--bazel-startup-option` tokens before `query`. It gives up after about
1.5 seconds or when another Bazel command holds the server, and then offers
package patterns only. A cold `bazel query` can start a Bazel server that
keeps running after completion gives up. On Windows that server outlives
the terminal or tool that ran dx, unless that tool's job forbids breakaway.
When dx runs without a console (started detached), completion that gives up
also stops that server. The query's client and any wrapper such as Bazelisk
stop when completion gives up. Completion prints no errors while you type.

Output: text only. `--output=json` and `--output=diff` are usage errors.
`DX_OUTPUT=json` and `output = "json"` in `dx.toml` fall back to text.

Exit codes: `0` success, `2` usage errors (missing or unknown shell, rejected
flags) or an invalid `dx.toml`, `1` stdout could not be written, `141` stdout
was closed early.

```sh
bazel run @rules_dx//:dx -- completion bash > ~/.cache/dx-completion.bash
```

To load completion in every new shell:

- Bash: add `source <(dx completion bash)` to `~/.bashrc`.
- Zsh: add `source <(dx completion zsh)` to `~/.zshrc` after `compinit`.
- Fish: add `dx completion fish | source` to `~/.config/fish/config.fish`.
- PowerShell: add `dx completion powershell | Out-String | Invoke-Expression`
  to `$PROFILE`.

The script calls `dx` from your `PATH`.
