# `dx completion`

```text
dx completion [<shell>] [--check]
```

Prints a static completion script for one shell: `bash`, `zsh`, `fish`, or
`powershell`.

- With one shell and no `--check`: print that shell script.
- With `--check`: verify without writing. Zero shells checks all shells.
- Unknown shells fail with `unknown-shell`.

Output: `--output text`.

Exit codes: `0` success, `2` usage errors including an unknown shell, `1` a
completion check failed.

```sh
bazel run //cli/cli:dx -- completion bash > ~/.cache/dx-completion.bash
bazel run //cli/cli:dx -- completion --check
```
