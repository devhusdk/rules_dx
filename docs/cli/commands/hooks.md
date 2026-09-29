# `dx init` And `dx hooks`

```sh
bazel run //cli/cli:dx -- init
bazel run //cli/cli:dx -- hooks install
```

## `dx init`

```text
dx init [module-name]
```

Scaffolds `dx` into a foreign tree. Takes an optional module name. Never
overwrites existing files.

Exit codes: `0` success, `2` usage errors including extra positionals, `1`
scaffolding failed.

## `dx hooks`

```text
dx hooks <install|uninstall|status|run [pre-commit|pre-push]>
```

Manages Git hooks through hermetic Git.

- `install`: install `pre-commit` and `pre-push` shims.
- `uninstall`: remove them.
- `status`: show what would run.
- `run <trigger>`: run one trigger.

Exit codes: `0` success, `2` usage errors including an unknown verb or
trigger, `1` a hook install, status, or check failed. A failing check reports
`1`, never the check's own code.

```sh
bazel run //cli/cli:dx -- hooks status
bazel run //cli/cli:dx -- hooks run pre-commit
```
