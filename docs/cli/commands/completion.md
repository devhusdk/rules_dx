# `dx completion`

Use `dx completion` to get Tab completion for dx commands, flags and Bazel targets in your shell.

```sh
source <(dx completion bash)
source <(dx completion zsh)
dx completion fish | source
dx completion powershell | Out-String | Invoke-Expression
dx completion bash > ~/.cache/dx-completion.bash
```

`dx completion` prints the completion script for one shell. It works outside a workspace too.

## Load it in every new shell

- Bash: add `source <(dx completion bash)` to `~/.bashrc`.
- Zsh: add `source <(dx completion zsh)` to `~/.zshrc`, after `compinit`.
- Fish: add `dx completion fish | source` to `~/.config/fish/config.fish`.
- PowerShell: add `dx completion powershell | Out-String | Invoke-Expression` to `$PROFILE`.

Keep `dx` on your `PATH`. The script asks `dx` for candidates as you type.

## Usage

```text
dx completion <shell>
```

`<shell>` is one of `bash`, `zsh`, `fish` or `powershell`.

## Flags

| Flag | What it does |
| --- | --- |
| `--workspace <dir>` | Use this workspace directory. |
| `-q, --quiet` | Hide summaries. |
| `-v, --verbose` | Show more logs. |
| `--color auto\|always\|never` | Set color output. |
| `--build-slots <auto\|n\|off>` | Set build slots: `auto`, a count, or `off`. |
| `--build-slots-wait <duration>` | Give up waiting for a build slot after this long. `0` fails at once. |
| `--bazel-startup-option <token>` | Pass one Bazel startup option. Repeatable. |

`dx completion` takes no other flag. `--output=json`, `--diff` and `-- bazel-options` are usage errors. `DX_OUTPUT=json` falls back to text.

## Scopes

Exactly one shell. Any other value fails with `unknown shell`.

## Candidates

As you type, completion offers:

- every command and every flag;
- the values each command takes in each position;
- the Bazel packages of the workspace, as labels such as `//pkg`. A *package* is a directory with a `BUILD.bazel` file;
- the targets of one package, such as `//pkg:name`. A *target* is one thing Bazel can build, run or test.

The workspace is the one `--workspace` names, else the one you are in.

To list the targets of a package, completion runs one `bazel query`. It gives up after about 1.5 seconds, or when another Bazel command holds the Bazel server, and offers package patterns only. Completion prints no errors while you type.

## Output

Text only. `dx completion` writes no reports.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | stdout could not be written. |
| `2` | Missing or unknown shell, a rejected flag, or an invalid `DX_` variable. |
| `141` | stdout closed early. |
