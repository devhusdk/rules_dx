# `dx status` And `dx version`

```sh
bazel run //cli/cli:dx -- status
bazel run //cli/cli:dx -- version
```

## `dx status`

```text
dx status
```

Reports toolchain, platform, tools, and pin drift. Takes no scopes and no
per-command flags. Output is text or JSON. There is no `dx doctor`. Use
`dx status` instead.

## `dx version`

```text
dx version [--check] [--pin <version>|--rollback]
```

Prints the version.

- `--check`: verify the pin without changing it.
- `--pin <version>`: re-pin to this version.
- `--rollback`: restore the last pin. Conflicts with `--pin`.

```sh
bazel run //cli/cli:dx -- version --check
bazel run //cli/cli:dx -- version --pin 0.0.0
bazel run //cli/cli:dx -- version --rollback
```
