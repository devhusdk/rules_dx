# Supported surface

This page lists every `dx` command, flag, argument and environment variable, every build setting, platform and toolchain type rules_dx makes public, and the version of each file format dx reads or writes. Use it to check what your scripts, CI and `BUILD` files can rely on.

This page is generated. Do not edit it by hand. Run `bazel run //tools/api:update` to refresh it.

## How to read the tables

- "Values" lists what a flag or argument accepts when it is a fixed set. An empty cell means free text, or a switch that takes no value.
- "Default" is the value dx uses when you leave the flag or argument out.
- "Also" lists the other spellings of a flag.
- A build setting is a flag you pass to Bazel as `--<setting>=<value>`. For an `alias`, "Default" names the setting it forwards to.

## Commands

### `dx`

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--version` | `-V` |  |  |

### `dx security`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGETS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--accept` |  |  |  |
| `--affected` |  |  |  |
| `--base` |  |  |  |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--bundle` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--expires` |  |  |  |
| `--fail-on` |  | `info`, `warning`, `error` |  |
| `--frozen` |  |  |  |
| `--here` |  |  |  |
| `--offline` |  |  |  |
| `--output` | `-o` | `text`, `json` |  |
| `--quiet` | `-q` |  |  |
| `--reason` |  |  |  |
| `--repo` |  |  |  |
| `--report` |  |  |  |
| `--since` |  |  |  |
| `--verbose` | `-v` |  |  |
| `--verify` |  |  |  |
| `--workspace` |  |  |  |

### `dx license`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGETS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--accept` |  |  |  |
| `--affected` |  |  |  |
| `--base` |  |  |  |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--expires` |  |  |  |
| `--fail-on` |  | `info`, `warning`, `error` |  |
| `--frozen` |  |  |  |
| `--here` |  |  |  |
| `--offline` |  |  |  |
| `--output` | `-o` | `text`, `json` |  |
| `--quiet` | `-q` |  |  |
| `--reason` |  |  |  |
| `--report` |  |  |  |
| `--since` |  |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx lint`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGETS` | no | yes |  |  |
| `BAZEL_OPTIONS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--affected` |  |  |  |
| `--apply` |  |  |  |
| `--base` |  |  |  |
| `--bazel-startup-option` |  |  |  |
| `--breaking-against` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--diff` |  |  |  |
| `--fail-on` |  | `info`, `warning`, `error` |  |
| `--here` |  |  |  |
| `--include-edits` |  |  |  |
| `--max-findings` |  |  |  |
| `--online` |  |  |  |
| `--output` | `-o` | `text`, `json` |  |
| `--quiet` | `-q` |  |  |
| `--report` |  |  |  |
| `--since` |  |  |  |
| `--unsafe` |  |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx typecheck`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGETS` | no | yes |  |  |
| `BAZEL_OPTIONS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--affected` |  |  |  |
| `--apply` |  |  |  |
| `--base` |  |  |  |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--diff` |  |  |  |
| `--fail-on` |  | `info`, `warning`, `error` |  |
| `--here` |  |  |  |
| `--include-edits` |  |  |  |
| `--max-findings` |  |  |  |
| `--output` | `-o` | `text`, `json` |  |
| `--quiet` | `-q` |  |  |
| `--report` |  |  |  |
| `--since` |  |  |  |
| `--unsafe` |  |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx format`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGETS` | no | yes |  |  |
| `BAZEL_OPTIONS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--affected` |  |  |  |
| `--apply` |  |  |  |
| `--base` |  |  |  |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--diff` |  |  |  |
| `--fail-on` |  | `info`, `warning`, `error` |  |
| `--here` |  |  |  |
| `--include-edits` |  |  |  |
| `--max-findings` |  |  |  |
| `--output` | `-o` | `text`, `json` |  |
| `--quiet` | `-q` |  |  |
| `--since` |  |  |  |
| `--unsafe` |  |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx generate`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGETS` | no | yes |  |  |
| `BAZEL_OPTIONS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--affected` |  |  |  |
| `--apply` |  |  |  |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--diff` |  |  |  |
| `--here` |  |  |  |
| `--output` | `-o` | `text`, `json` |  |
| `--quiet` | `-q` |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx build`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGETS` | no | yes |  |  |
| `BAZEL_OPTIONS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--affected` |  |  |  |
| `--base` |  |  |  |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--debug` |  |  |  |
| `--here` |  |  |  |
| `--no-retry` |  |  |  |
| `--offline` |  |  |  |
| `--output` | `-o` | `text`, `json` |  |
| `--quiet` | `-q` |  |  |
| `--release` |  |  |  |
| `--report` |  |  |  |
| `--since` |  |  |  |
| `--timings` |  |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx test`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGETS` | no | yes |  |  |
| `BAZEL_OPTIONS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--affected` |  |  |  |
| `--base` |  |  |  |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--debug` |  |  |  |
| `--here` |  |  |  |
| `--no-retry` |  |  |  |
| `--offline` |  |  |  |
| `--output` | `-o` | `text`, `json` |  |
| `--quiet` | `-q` |  |  |
| `--release` |  |  |  |
| `--report` |  |  |  |
| `--since` |  |  |  |
| `--timings` |  |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx coverage`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGETS` | no | yes |  |  |
| `BAZEL_OPTIONS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--affected` |  |  |  |
| `--base` |  |  |  |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--fail-on` |  | `info`, `warning`, `error` |  |
| `--here` |  |  |  |
| `--min-changed-line-coverage` |  |  |  |
| `--min-coverage` |  |  |  |
| `--no-retry` |  |  |  |
| `--output` | `-o` | `text`, `json` |  |
| `--quiet` | `-q` |  |  |
| `--report` |  |  |  |
| `--since` |  |  |  |
| `--timings` |  |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx bench`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGETS` | no | yes |  |  |
| `BAZEL_OPTIONS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--compare` |  |  |  |
| `--here` |  |  |  |
| `--max-regression` |  |  |  |
| `--output` | `-o` | `text`, `json` |  |
| `--quiet` | `-q` |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx run`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGET` | no | yes |  |  |
| `APP_ARGS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--bazel-option` |  |  |  |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--debug` |  |  |  |
| `--output` | `-o` | `text`, `json` |  |
| `--quiet` | `-q` |  |  |
| `--release` |  |  |  |
| `--timings` |  |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx deploy`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGET` | no | yes |  |  |
| `APP_ARGS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--affected` |  |  |  |
| `--apply` |  |  |  |
| `--base` |  |  |  |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--debug` |  |  |  |
| `--quiet` | `-q` |  |  |
| `--release` |  |  |  |
| `--since` |  |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx check`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGETS` | no | yes |  |  |
| `BAZEL_OPTIONS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--affected` |  |  |  |
| `--apply` |  |  |  |
| `--base` |  |  |  |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--diff` |  |  |  |
| `--fail-on` |  | `info`, `warning`, `error` |  |
| `--here` |  |  |  |
| `--include-edits` |  |  |  |
| `--max-findings` |  |  |  |
| `--no-retry` |  |  |  |
| `--output` | `-o` | `text`, `json` |  |
| `--quiet` | `-q` |  |  |
| `--report` |  |  |  |
| `--since` |  |  |  |
| `--unsafe` |  |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx clean`

Exit codes: `0`, `1`, `2`.

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--bazel` |  |  |  |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--expunge` |  |  |  |
| `--output` | `-o` | `text`, `json` |  |
| `--quiet` | `-q` |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx update`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGETS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--add-missing` |  |  |  |
| `--allow-major` |  |  |  |
| `--apply` |  |  |  |
| `--automerge` |  | `none`, `patch`, `minor`, `all` |  |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--fail-on` |  | `info`, `warning`, `error` |  |
| `--floor` |  |  |  |
| `--frozen` |  |  |  |
| `--max-prs` |  |  |  |
| `--min-age` |  |  |  |
| `--offline` |  |  |  |
| `--outdated` |  |  |  |
| `--output` | `-o` | `text`, `json` |  |
| `--pr` |  |  |  |
| `--prune` |  |  |  |
| `--quiet` | `-q` |  |  |
| `--reason` |  |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx sync`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGETS` | no | yes |  |  |
| `BAZEL_OPTIONS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--output` | `-o` | `text`, `json` |  |
| `--quiet` | `-q` |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx init`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGETS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--accept-license` |  |  |  |
| `--agents` |  | `claude`, `codex`, `gemini`, `copilot` |  |
| `--apply` |  |  |  |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--ci` |  | `github`, `gitlab`, `none` |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--no-generate` |  |  |  |
| `--output` | `-o` | `text`, `json` |  |
| `--quiet` | `-q` |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx upgrade`

Exit codes: `0`, `1`, `2`.

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--apply` |  |  |  |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--output` | `-o` | `text`, `json` |  |
| `--quiet` | `-q` |  |  |
| `--to` |  |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx status`

Exit codes: `0`, `1`, `2`.

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--capabilities` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--files` |  |  |  |
| `--output` | `-o` | `text`, `json` |  |
| `--quiet` | `-q` |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx watch`

Exit codes: `129`, `130`, `141`, `143`, `2`, `3`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGETS` | no | yes |  |  |
| `BAZEL_OPTIONS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--affected` |  |  |  |
| `--apply` |  |  |  |
| `--base` |  |  |  |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--debug` |  |  |  |
| `--diff` |  |  |  |
| `--fail-on` |  | `info`, `warning`, `error` |  |
| `--here` |  |  |  |
| `--include-edits` |  |  |  |
| `--max-findings` |  |  |  |
| `--output` | `-o` | `text`, `json` |  |
| `--poll` |  |  |  |
| `--quiet` | `-q` |  |  |
| `--release` |  |  |  |
| `--report` |  |  |  |
| `--since` |  |  |  |
| `--unsafe` |  |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx completion`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGETS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--quiet` | `-q` |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx docs`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGETS` | no | yes |  |  |
| `BAZEL_OPTIONS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--here` |  |  |  |
| `--host` |  |  |  |
| `--open` |  |  |  |
| `--output` | `-o` | `text`, `json` |  |
| `--port` |  |  |  |
| `--quiet` | `-q` |  |  |
| `--serve` |  |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx worktree`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGETS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--apply` |  |  |  |
| `--bases` |  |  |  |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--create` |  |  |  |
| `--disk` |  |  |  |
| `--force` |  |  |  |
| `--output` | `-o` | `text`, `json` |  |
| `--path-only` |  |  |  |
| `--quiet` | `-q` |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx fuzz`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGETS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--bazel-startup-option` |  |  |  |
| `--budget` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--here` |  |  |  |
| `--jobs` |  |  |  |
| `--output` | `-o` | `text`, `json` |  |
| `--quiet` | `-q` |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx config`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGETS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--output` | `-o` | `text`, `json` |  |
| `--quiet` | `-q` |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

### `dx fetch`

Exit codes: `0`, `1`, `2`.

| Argument | Required | Repeats | Values | Default |
| --- | --- | --- | --- | --- |
| `TARGETS` | no | yes |  |  |
| `BAZEL_OPTIONS` | no | yes |  |  |

| Flag | Also | Values | Default |
| --- | --- | --- | --- |
| `--affected` |  |  |  |
| `--base` |  |  |  |
| `--bazel-startup-option` |  |  |  |
| `--build-slots` |  |  |  |
| `--build-slots-wait` |  |  |  |
| `--color` |  | `auto`, `always`, `never` |  |
| `--debug` |  |  |  |
| `--here` |  |  |  |
| `--output` | `-o` | `text`, `json` |  |
| `--quiet` | `-q` |  |  |
| `--release` |  |  |  |
| `--since` |  |  |  |
| `--vendor` |  |  |  |
| `--verbose` | `-v` |  |  |
| `--workspace` |  |  |  |

## Environment variables

| Variable | Value | Default for |
| --- | --- | --- |
| `DX_OUTPUT` | `mode` | `--output` |
| `DX_BUILD_SLOTS` | `auto\|n\|off` | `--build-slots` |
| `DX_BUILD_SLOTS_WAIT` | `duration` | `--build-slots-wait` |
| `NO_COLOR` | `non-empty` |  |
| `BUILD_WORKSPACE_DIRECTORY` | `dir` |  |
| `BUILD_WORKING_DIRECTORY` | `dir` |  |

## Build settings

| Setting | Kind | Default | Values |
| --- | --- | --- | --- |
| `@rules_dx//config:buf_breaking_against` | `label_flag` | `@rules_dx//config:buf_breaking_none` |  |
| `@rules_dx//config:clippy_flag` | `alias` | `@rules_rust//rust/settings:clippy_flag` |  |
| `@rules_dx//config:clippy_output_diagnostics` | `alias` | `@rules_rust//rust/settings:clippy_output_diagnostics` |  |
| `@rules_dx//config:config_tier` | `string_flag` | `standard` | `standard`, `strict` |
| `@rules_dx//config:extra_rustc_flag` | `alias` | `@rules_rust//rust/settings:extra_rustc_flag` |  |
| `@rules_dx//config:rustc_output_diagnostics` | `alias` | `@rules_rust//rust/settings:rustc_output_diagnostics` |  |
| `@rules_dx//config:tool_actionlint` | `label_flag` | `@dx_tools//:actionlint` |  |
| `@rules_dx//config:tool_avro` | `label_flag` | `@rules_dx//java/private/quality/tools:avro_tools` |  |
| `@rules_dx//config:tool_capnp` | `label_flag` | `@dx_prebuilt//:capnp` |  |
| `@rules_dx//config:tool_container_structure_test` | `label_flag` | `@dx_tools//:container_structure_test` |  |
| `@rules_dx//config:tool_credo` | `label_flag` | `@rules_dx//elixir/private/tools:credo` |  |
| `@rules_dx//config:tool_dart` | `label_flag` | `@rules_dx//config:no_dart` |  |
| `@rules_dx//config:tool_dialyzer` | `label_flag` | `@rules_dx//erlang/private/tools:dialyzer` |  |
| `@rules_dx//config:tool_elvis` | `label_flag` | `@rules_dx//erlang/private/tools:elvis` |  |
| `@rules_dx//config:tool_erlfmt` | `label_flag` | `@rules_dx//erlang/private/tools:erlfmt` |  |
| `@rules_dx//config:tool_errcheck` | `label_flag` | `@dx_prebuilt//:dx_errcheck` |  |
| `@rules_dx//config:tool_flatc` | `label_flag` | `@dx_prebuilt//:flatc` |  |
| `@rules_dx//config:tool_gersemi` | `label_flag` | `@dx_tools//:gersemi` |  |
| `@rules_dx//config:tool_gofumpt` | `label_flag` | `@cc_mvdan_gofumpt//:gofumpt` |  |
| `@rules_dx//config:tool_gotype` | `label_flag` | `@dx_prebuilt//:dx_gotype` |  |
| `@rules_dx//config:tool_govet` | `label_flag` | `@dx_prebuilt//:dx_govet` |  |
| `@rules_dx//config:tool_hadolint` | `label_flag` | `@dx_tools//:hadolint` |  |
| `@rules_dx//config:tool_helm` | `label_flag` | `@dx_tools//:helm` |  |
| `@rules_dx//config:tool_helmfmt` | `label_flag` | `@dx_tools//:helmfmt` |  |
| `@rules_dx//config:tool_hlint` | `label_flag` | `@rules_dx//haskell/private/tools/hlint/bin` |  |
| `@rules_dx//config:tool_jet` | `label_flag` | `@rules_dx//julia/private/tools/jet:jet` |  |
| `@rules_dx//config:tool_julia_format` | `label_flag` | `@rules_dx//config:no_julia_format` |  |
| `@rules_dx//config:tool_julialint` | `label_flag` | `@rules_dx//julia/private/tools/lint:julialint` |  |
| `@rules_dx//config:tool_kubeconform` | `label_flag` | `@dx_tools//:kubeconform` |  |
| `@rules_dx//config:tool_luals` | `label_flag` | `@rules_dx//lua/private/tools:luals` |  |
| `@rules_dx//config:tool_mix_format` | `label_flag` | `@rules_dx//elixir/private/tools:mix_format` |  |
| `@rules_dx//config:tool_modfmt` | `label_flag` | `@dx_prebuilt//:dx_modfmt` |  |
| `@rules_dx//config:tool_nixfmt` | `label_flag` | `@rules_dx//core/quality/tools/nixfmt/bin:nixfmt` |  |
| `@rules_dx//config:tool_opa` | `label_flag` | `@dx_tools//:opa` |  |
| `@rules_dx//config:tool_ormolu` | `label_flag` | `@rules_dx//haskell/private/tools/ormolu/bin` |  |
| `@rules_dx//config:tool_perlcritic` | `label_flag` | `@rules_dx//config:no_perlcritic` |  |
| `@rules_dx//config:tool_perltidy` | `label_flag` | `@rules_dx//config:no_perltidy` |  |
| `@rules_dx//config:tool_phpcs` | `label_flag` | `@rules_dx//php/private/tools:phpcs` |  |
| `@rules_dx//config:tool_phpstan` | `label_flag` | `@rules_dx//php/private/tools:phpstan` |  |
| `@rules_dx//config:tool_pkl` | `label_flag` | `@dx_tools//:pkl` |  |
| `@rules_dx//config:tool_qmlformat` | `label_flag` | `@rules_dx//config:no_qmlformat` |  |
| `@rules_dx//config:tool_qmllint` | `label_flag` | `@rules_dx//config:no_qmllint` |  |
| `@rules_dx//config:tool_regal` | `label_flag` | `@dx_tools//:regal` |  |
| `@rules_dx//config:tool_ruff` | `label_flag` | `@dx_tools//:ruff` |  |
| `@rules_dx//config:tool_sorbet` | `label_flag` | `@rules_dx//ruby/private/tools:sorbet` |  |
| `@rules_dx//config:tool_staticcheck` | `label_flag` | `@dx_prebuilt//:dx_staticcheck` |  |
| `@rules_dx//config:tool_stylua` | `label_flag` | `@dx_tools//:stylua` |  |
| `@rules_dx//config:tool_swift_format` | `label_flag` | `@rules_dx//config:no_swift_format` |  |
| `@rules_dx//config:tool_swift_lint` | `label_flag` | `@rules_dx//config:no_swift_lint` |  |
| `@rules_dx//config:tool_terraform` | `label_flag` | `@dx_tools//:terraform` |  |
| `@rules_dx//config:tool_thrift` | `label_flag` | `@dx_prebuilt//:thrift` |  |
| `@rules_dx//config:tool_trivy` | `label_flag` | `@dx_tools//:trivy` |  |
| `@rules_dx//config:tool_weeder` | `label_flag` | `@rules_dx//haskell/private/tools/weeder/bin` |  |
| `@rules_dx//config:tool_zig_ast_check` | `label_flag` | `@rules_dx//config:no_zig_ast_check` |  |
| `@rules_dx//config:tool_zig_fmt` | `label_flag` | `@rules_dx//config:no_zig_fmt` |  |
| `@rules_dx//config:toolchain_linker_preference` | `alias` | `@rules_rust//rust/settings:toolchain_linker_preference` |  |
| `@rules_dx//config:toolchains` | `bool_flag` | `False` |  |
| `@rules_dx//config:warnings_as_errors` | `bool_flag` | `True` |  |
| `@rules_dx//config:workspace` | `label_flag` | `@rules_dx//config:default_workspace_policy` |  |
| `@rules_dx@*` | `hub` | `@rules_dx//go:extensions.bzl%go` |  |
| `@rules_dx@ansible_collections` | `hub` | `@rules_dx//ansible:extensions.bzl%ansible` |  |
| `@rules_dx@bicep` | `hub` | `@rules_dx//bicep:extensions.bzl%bicep` |  |
| `@rules_dx@composer` | `hub` | `@rules_dx//php:extensions.bzl%composer` |  |
| `@rules_dx@cpan` | `hub` | `@rules_dx//perl:extensions.bzl%cpan` |  |
| `@rules_dx@crates` | `hub` | `@rules_dx//rust:extensions.bzl%cargo` |  |
| `@rules_dx@dub` | `hub` | `@rules_dx//d:extensions.bzl%dub` |  |
| `@rules_dx@dx_terraform_providers` | `hub` | `@rules_dx//terraform:extensions.bzl%terraform` |  |
| `@rules_dx@fpm` | `hub` | `@rules_dx//fortran:extensions.bzl%fpm` |  |
| `@rules_dx@gems` | `hub` | `@rules_dx//ruby:extensions.bzl%ruby` |  |
| `@rules_dx@hackage` | `hub` | `@rules_dx//haskell:extensions.bzl%cabal` |  |
| `@rules_dx@hex` | `hub` | `@rules_dx//elixir:extensions.bzl%hex` |  |
| `@rules_dx@julia` | `hub` | `@rules_dx//julia:extensions.bzl%julia` |  |
| `@rules_dx@maven` | `hub` | `@rules_dx//java:extensions.bzl%maven` |  |
| `@rules_dx@npm` | `hub` | `@rules_dx//javascript:extensions.bzl%npm` |  |
| `@rules_dx@nuget` | `hub` | `@rules_dx//csharp:extensions.bzl%nuget` |  |
| `@rules_dx@opam` | `hub` | `@rules_dx//ocaml:extensions.bzl%opam` |  |
| `@rules_dx@pnpm` | `hub` | `@rules_dx//javascript:extensions.bzl%pnpm` |  |
| `@rules_dx@psgallery` | `hub` | `@rules_dx//powershell:extensions.bzl%powershell` |  |
| `@rules_dx@pub_deps` | `hub` | `@rules_dx//dart:extensions.bzl%pub` |  |
| `@rules_dx@pypi` | `hub` | `@rules_dx//python:extensions.bzl%uv` |  |
| `@rules_dx@rocks` | `hub` | `@rules_dx//lua:extensions.bzl%luarocks` |  |
| `@rules_dx@swiftpkg` | `hub` | `@rules_dx//swift:extensions.bzl%swiftpm` |  |
| `@rules_dx@zig` | `hub` | `@rules_dx//zig:extensions.bzl%zig` |  |

## Platforms

Pass a platform to Bazel with `--platforms=<platform>`.

- `@rules_dx//android:android_device`
- `@rules_dx//android:android_emulator`
- `@rules_dx//android:arm64-v8a`
- `@rules_dx//android:x86_64`
- `@rules_dx//apple:macos_arm64`
- `@rules_dx//apple:macos_x86_64`
- `@rules_dx//msix:windows_arm64`
- `@rules_dx//msix:windows_x86_32`
- `@rules_dx//msix:windows_x86_64`
- `@rules_dx//qt:linux_arm64`
- `@rules_dx//qt:linux_x86_64`
- `@rules_dx//qt:macos_arm64`
- `@rules_dx//qt:windows_arm64`
- `@rules_dx//qt:windows_x86_64`

## Toolchain types

Use a toolchain type when you register your own toolchain.

- `@rules_dx//erlang:toolchain_type`
- `@rules_dx//lua:toolchain_type`
- `@rules_dx//msix:toolchain_type`
- `@rules_dx//php:exec_toolchain_type`
- `@rules_dx//php:toolchain_type`

## File formats

| Schema | Version |
| --- | --- |
| `events.schema.json` | `4.0` |
| `dx.schema.json` | `1.2` |
