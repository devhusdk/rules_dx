# `dx completion`

Prints the completion script for one shell. Runs outside a workspace.

```sh
dx completion bash > ~/.cache/dx-completion.bash
source <(dx completion bash)
source <(dx completion zsh)
dx completion fish | source
dx completion powershell | Out-String | Invoke-Expression
```

```text
dx completion <shell>
```

Shells: `bash`, `zsh`, `fish`, `powershell`.

To load completion in every new shell:

- Bash: add `source <(dx completion bash)` to `~/.bashrc`.
- Zsh: add `source <(dx completion zsh)` to `~/.zshrc` after `compinit`.
- Fish: add `dx completion fish | source` to `~/.config/fish/config.fish`.
- PowerShell: add `dx completion powershell | Out-String | Invoke-Expression` to `$PROFILE`.

Keep `dx` on your `PATH`. The script asks `dx` for candidates as you type.

## Flags

None. `--output=json`, `--diff`, and `-- bazel-options` are usage errors. `DX_OUTPUT=json` falls back to text.

Scopes: exactly one shell. Unknown shells fail with `unknown shell`.

## Candidates

Candidates are every command, every flag, the per-command slots, the package labels of the workspace, and the targets of one package. The workspace is `--workspace`, then the current workspace.

Target completion runs one `bazel query` for the typed package. It gives up after about 1.5 seconds or when another Bazel command holds the server, then offers package patterns only. Completion prints no errors while you type.

## Output

Text only. Reports: none.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | stdout could not be written. |
| `2` | Missing or unknown shell, a rejected flag, or an invalid `DX_` variable. |
| `141` | stdout closed early. |
