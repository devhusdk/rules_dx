# Supported surface

This page lists every `dx` command, flag, argument and environment variable, every `.bzl` file you can load with its rules, macros, attributes, providers and module extensions, every build setting, platform, toolchain type, hub repository and target rules_dx makes public, and the version of each file format dx reads or writes. Use it to check what your scripts, CI, `MODULE.bazel` and `BUILD` files can rely on.

This page is generated. Do not edit it by hand. Run `bazel run //tools/api:update` to refresh it.

## How to read the tables

- "Values" lists what a flag or argument accepts when it is a fixed set. An empty cell means free text, or a switch that takes no value.
- "Default" is the value dx uses when you leave the flag or argument out.
- "Also" lists the other spellings of a flag.
- A build setting is a flag you pass to Bazel as `--<setting>=<value>`. For an `alias`, "Default" names the setting it forwards to.
- `pinned` in "Default" means the tool rules_dx pins. Set the flag or argument to your own label to replace it.
- An attribute is a named argument of a rule, macro or tag. "Required" says whether you must set it.

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
| `--lint-scope` |  | `direct`, `closure` |  |
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
| `--coverage` |  |  |  |
| `--diff` |  |  |  |
| `--fail-on` |  | `info`, `warning`, `error` |  |
| `--here` |  |  |  |
| `--include-edits` |  |  |  |
| `--lint-scope` |  | `direct`, `closure` |  |
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

## Starlark files

Load a symbol in a `BUILD` or `.bzl` file with `load("<file>", "<symbol>")`. Use a module extension in `MODULE.bazel` with `use_extension("<file>", "<extension>")`.

### `@rules_dx//android:defs.bzl`

#### `android_apk_set`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `aab` | `label` | yes |  |
| `device_spec` | `label` | yes |  |
| `name` | `name` | yes |  |

#### `android_adb_install`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `apk` | no | `None` |
| `serial` | no | `""` |
| `apk_set` | no | `None` |
| `visibility` | no | `None` |

#### `android_adb_log`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `serial` | no | `""` |
| `visibility` | no | `None` |

#### `android_adb_start`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `app_id` | yes |  |
| `activity` | no | `"android.app.NativeActivity"` |
| `serial` | no | `""` |
| `visibility` | no | `None` |

#### `android_check`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `package` | yes |  |
| `platforms` | no | `[]` |
| `resources` | no | `[]` |
| `size` | no | `"small"` |
| `tags` | no | `None` |
| `target_compatible_with` | no | `None` |
| `visibility` | no | `None` |

#### `android_java_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `kwargs` | no |  |

#### `android_kotlin_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `kwargs` | no |  |

#### `android_native_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `rust_srcs` | yes |  |
| `c_srcs` | yes |  |
| `hdrs` | no | `[]` |
| `crate_name` | no | `None` |
| `edition` | no | `"2024"` |
| `platforms` | no | `None` |
| `api_level` | no | `31` |
| `crate_features` | no | `None` |
| `rustc_flags` | no | `[]` |
| `copts` | no | `[]` |
| `linkopts` | no | `[]` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `android_rust_apk`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `app_id` | yes |  |
| `deps` | yes |  |
| `platforms` | no | `[Label("@rules_dx//android:arm64-v8a"), Label("@rules_dx//android:x86_64")]` |
| `activity` | no | `"android.app.NativeActivity"` |
| `api_level` | no | `0` |
| `assets` | no | `[]` |
| `assets_dir` | no | `""` |
| `custom_package` | no | `""` |
| `manifest` | no | `None` |
| `permissions` | no | `[]` |
| `resource_files` | no | `[]` |
| `version_code` | no | `1` |
| `version_name` | no | `"1.0"` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `android_sign`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `package` | yes |  |
| `schemes` | no | `None` |
| `min_sdk` | no | `0` |
| `visibility` | no | `None` |

### `@rules_dx//android:extensions.bzl`

#### `android`

Module extension.

Tag `android.configure`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `api_level` | `int` | no | `0` |
| `enabled` | `boolean` | no | `True` |
| `extra_packages` | `string_list` | no | `[]` |
| `mirror` | `string` | no | `""` |

#### `android_sdk_repository`

Module extension.

Tag `android_sdk_repository.configure`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `api_level` | `int` | no | `0` |
| `build_tools_version` | `string` | no | `""` |
| `path` | `string` | no | `""` |

### `@rules_dx//ansible:defs.bzl`

#### `ansible_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `playbooks` | `label_list` | no | `[]` |
| `srcs` | `label_list` | no | `[]` |

### `@rules_dx//ansible:extensions.bzl`

#### `ansible`

Module extension.

Tag `ansible.lock`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `lock` | `label` | yes |  |
| `name` | `name` | yes |  |

### `@rules_dx//api_spec:defs.bzl`

#### `api_spec_go_client`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `codegen_path` | `string` | no | `""` |
| `name` | `name` | yes |  |
| `out` | `string` | no | `""` |
| `package` | `string` | yes |  |
| `root` | `label` | no | `None` |
| `spec` | `label` | yes |  |

#### `api_spec_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `refs` | `label_list` | no | `[]` |
| `srcs` | `label_list` | no | `[]` |

#### `api_spec_python_client`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `codegen_path` | `string` | no | `""` |
| `name` | `name` | yes |  |
| `out` | `string` | no | `""` |
| `root` | `label` | no | `None` |
| `spec` | `label` | yes |  |

#### `api_spec_rust_client`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `codegen_path` | `string` | no | `""` |
| `name` | `name` | yes |  |
| `out` | `string` | no | `""` |
| `root` | `label` | no | `None` |
| `spec` | `label` | yes |  |

#### `api_spec_typescript_client`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `codegen_path` | `string` | no | `""` |
| `name` | `name` | yes |  |
| `out` | `string` | no | `""` |
| `root` | `label` | no | `None` |
| `spec` | `label` | yes |  |

#### `api_spec_protobuf_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `proto` | yes |  |
| `package` | no | `""` |
| `kwargs` | no |  |

#### `ApiSpecInfo`

Provider.

Fields: `files`, `roots`.

### `@rules_dx//appimage:defs.bzl`

#### `appimage_package`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `target_compatible_with` | no | `None` |
| `kwargs` | no |  |

#### `AppImageInfo`

Provider.

Fields: `identity`, `image`, `name`, `version`.

### `@rules_dx//appimage:extensions.bzl`

#### `appimage`

Module extension.

No tags.

### `@rules_dx//apple:defs.bzl`

#### `apple_dynamic_xcframework_import`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `bundle_only` | `boolean` | no | `False` |
| `deps` | `label_list` | no | `[]` |
| `library_identifiers` | `string_dict` | no | `{}` |
| `name` | `name` | yes |  |
| `xcframework_imports` | `label_list` | yes |  |

#### `apple_ios_app`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `library` | no | `None` |
| `frameworks` | no | `[]` |
| `entry` | no | `None` |
| `srcs` | no | `None` |
| `hdrs` | no | `[]` |
| `deps` | no | `None` |
| `platform` | no | `None` |
| `bundle_id` | no | `""` |
| `version` | no | `""` |
| `deployment` | no | `""` |
| `xcode_version` | no | `""` |
| `resources` | no | `[]` |
| `infoplists` | no | `[]` |
| `families` | no | `["iphone"]` |
| `entitlements` | no | `None` |
| `provisioning_profile` | no | `None` |
| `tags` | no | `None` |
| `testonly` | no | `False` |
| `visibility` | no | `None` |

#### `apple_ios_app_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `app` | yes |  |
| `srcs` | no | `None` |
| `deployment` | no | `""` |
| `device_type` | no | `""` |
| `os_version` | no | `""` |
| `deps` | no | `None` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `apple_ios_export`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `app` | yes |  |
| `visibility` | no | `None` |

#### `apple_ios_lib`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `[]` |
| `deps` | no | `[]` |
| `proc_macro_deps` | no | `[]` |
| `crate_root` | no | `None` |
| `crate_name` | no | `None` |
| `edition` | no | `None` |
| `hdrs` | no | `[]` |
| `platform` | no | `None` |
| `triple` | no | `""` |
| `sdk` | no | `""` |
| `deployment` | no | `""` |
| `xcode_version` | no | `""` |
| `crate_features` | no | `[]` |
| `rustc_flags` | no | `[]` |
| `frameworks` | no | `[]` |
| `tags` | no | `[]` |
| `testonly` | no | `False` |
| `visibility` | no | `None` |

#### `apple_ios_xcframework`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `library` | no | `None` |
| `hdrs` | no | `[]` |
| `deps` | no | `None` |
| `bundle_id` | no | `""` |
| `bundle_name` | no | `""` |
| `version` | no | `""` |
| `deployment` | no | `""` |
| `xcode_version` | no | `""` |
| `platforms` | no | `[]` |
| `resources` | no | `[]` |
| `infoplists` | no | `[]` |
| `tags` | no | `None` |
| `testonly` | no | `False` |
| `visibility` | no | `None` |

#### `apple_macos_app`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `hdrs` | no | `[]` |
| `deps` | no | `None` |
| `entry` | no | `None` |
| `library` | no | `None` |
| `frameworks` | no | `[]` |
| `plugins` | no | `[]` |
| `resources` | no | `[]` |
| `infoplists` | no | `[]` |
| `bundle_id` | no | `""` |
| `version` | no | `""` |
| `deployment` | no | `""` |
| `platform` | no | `None` |
| `xcode_version` | no | `""` |
| `entitlements` | no | `None` |
| `provisioning_profile` | no | `None` |
| `tags` | no | `None` |
| `testonly` | no | `None` |
| `visibility` | no | `None` |

#### `apple_macos_notarize`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `app` | yes |  |
| `visibility` | no | `None` |

#### `apple_macos_sign`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `app` | yes |  |
| `visibility` | no | `None` |

#### `apple_simulator_boot`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `simulator` | no | `""` |
| `visibility` | no | `None` |

#### `apple_simulator_install`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `app` | yes |  |
| `simulator` | no | `""` |
| `visibility` | no | `None` |

#### `apple_simulator_launch`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `bundle_id` | yes |  |
| `simulator` | no | `""` |
| `visibility` | no | `None` |

### `@rules_dx//apple:extensions.bzl`

#### `apple`

Module extension.

No tags.

### `@rules_dx//asciidoc:defs.bzl`

#### `asciidoc_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

### `@rules_dx//astro:defs.bzl`

#### `astro_build`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `config` | yes |  |
| `node_modules` | yes |  |
| `package_json` | no | `None` |
| `public` | no | `None` |
| `ssr_entry` | no | `None` |
| `deps` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `astro_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `tsconfig` | yes |  |
| `entry_points` | yes |  |
| `package` | yes |  |
| `node_modules` | yes |  |
| `package_json` | no | `None` |
| `deps` | no | `[]` |
| `kwargs` | no |  |

#### `astro_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `astro_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `node_modules` | yes |  |
| `data` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//avro:defs.bzl`

#### `avro_cc_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `codegen_path` | `string` | no | `""` |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `namespace` | `string` | no | `""` |

#### `avro_java_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `codegen_path` | `string` | no | `""` |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |

#### `avro_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

#### `avro_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `avro` | yes |  |
| `package` | no | `""` |
| `kwargs` | no |  |

#### `AvroInfo`

Provider.

Fields: `direct_deps`, `direct_srcs`, `label`, `transitive_srcs`.

### `@rules_dx//avro:extensions.bzl`

#### `avro_cc`

Module extension.

No tags.

### `@rules_dx//bicep:defs.bzl`

#### `bicep_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `data` | `label_list` | no | `[]` |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

#### `bicep_registry_module`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `cache_root` | `string` | no | `""` |
| `digest` | `string` | no | `""` |
| `name` | `name` | yes |  |
| `ref` | `string` | yes |  |
| `srcs` | `label_list` | no | `[]` |

### `@rules_dx//bicep:extensions.bzl`

#### `bicep`

Module extension.

Tag `bicep.lock`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `lock` | `label` | yes |  |
| `name` | `name` | yes |  |

### `@rules_dx//capnproto:defs.bzl`

#### `capnproto_cc_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `codegen_path` | `string` | no | `""` |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |

#### `capnproto_java_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `codegen_path` | `string` | no | `""` |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |

#### `capnproto_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

#### `capnproto_rust_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `library` | yes |  |
| `crate_name` | no | `None` |
| `deps` | no | `None` |
| `codegen_path` | no | `None` |
| `kwargs` | no |  |

#### `CapnprotoInfo`

Provider.

Fields: `direct_deps`, `direct_srcs`, `direct_staged`, `label`, `stage_root`, `stage_roots`, `staged`, `transitive_imports`, `transitive_srcs`.

### `@rules_dx//cc:defs.bzl`

#### `cc_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `deps` | no | `None` |
| `kwargs` | no |  |

#### `cc_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `cc_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `lib` | yes |  |
| `package` | no | `""` |
| `kwargs` | no |  |

#### `cc_fuzz_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `copts` | no | `None` |
| `linkopts` | no | `None` |
| `kwargs` | no |  |

#### `cc_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `hdrs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `cc_objc_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `cc_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//cc:extensions.bzl`

#### `cc`

Module extension.

Tag `cc.host_tools`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `hosts` | `string_list` | yes |  |

Tag `cc.msvc_runtime`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `architectures` | `string_list` | no | `["x64", "arm64"]` |
| `installer_manifest_integrity` | `string` | no | `""` |
| `installer_manifest_sha256` | `string` | no | `""` |
| `installer_manifest_url` | `string` | no | `""` |
| `msvc_version` | `string` | no | `""` |
| `pins` | `label` | no | `None` |
| `visual_studio_channel_url` | `string` | no | `"https://aka.ms/vs/stable/channel"` |
| `visual_studio_installer_manifest_integrity` | `string` | no | `""` |
| `visual_studio_installer_manifest_url` | `string` | no | `""` |

Tag `cc.windows_sdk`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `architectures` | `string_list` | no | `["x64", "arm64"]` |
| `pins` | `label` | no | `None` |
| `transformations` | `string_dict` | no | `{}` |
| `windows_sdk_integrity` | `string_dict` | no | `{}` |
| `windows_sdk_version` | `string` | no | `""` |

### `@rules_dx//cloudformation:defs.bzl`

#### `cloudformation_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

### `@rules_dx//cmake:defs.bzl`

#### `cmake_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

### `@rules_dx//core:defs.bzl`

#### `ansible_lint_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `bicep_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `biome_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `buf_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `buildifier_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `cfn_lint_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `checkmake_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `clang_format_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `clang_tidy_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `cmakelint_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `credo_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `csharpier_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `cue_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

#### `dart_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `detekt_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `djlint_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `docstrfmt_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `elvis_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `errcheck_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `eslint_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `ffmt_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `fortitude_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `fsharplint_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `gersemi_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `gherkin_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

#### `govet_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `gplint_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `graphql_eslint_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `hadolint_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `helmfmt_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `hlint_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `html_template_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

#### `jsonnet_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

#### `julialint_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `jupyter_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

#### `latex_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

#### `latexindent_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `luals_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `lychee_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `markdown_check_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `mbake_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `mix_format_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `nix_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

#### `perlcritic_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `perltidy_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `phpcs_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `phpstan_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `pkl_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

#### `pmd_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `prettier_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `psscriptanalyzer_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `pydoclint_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `qmlformat_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `qmllint_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `redocly_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `regal_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `remark_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `roslyn_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `rubocop_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `ruff_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `rustfmt_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `scalafix_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `scalafmt_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `selene_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `shellcheck_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `shfmt_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `spotbugs_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `sqlfluff_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `staticcheck_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `stylelint_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `stylua_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `swift_format_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `swift_lint_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `taplo_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `tflint_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `trivy_config_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `ty_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `typos_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `typst_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

#### `vale_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `xml_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

#### `yaml_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

#### `yamlfmt_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `yamllint_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `zizmor_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `docs_site`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `libraries` | yes |  |
| `prose` | yes |  |
| `book_toml` | yes |  |
| `prose_route` | no | `"prose.md"` |
| `source_url` | no | `""` |

#### `dx_generated_policy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `tier` | yes |  |
| `families` | no | `None` |
| `defaults` | no | `True` |
| `disabled` | no | `[]` |
| `configs` | no | `[]` |
| `bases` | no | `{}` |
| `visibility` | no | `None` |

#### `lua_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `lib` | yes |  |
| `package` | no | `""` |
| `kwargs` | no |  |

#### `php_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `lib` | yes |  |
| `package` | no | `""` |
| `kwargs` | no |  |

#### `DxNativeConfigInfo`

Provider.

Fields: `closure`, `config`, `tool_id`.

### `@rules_dx//core:extensions.bzl`

#### `dx_gazelle`

Module extension.

Tag `dx_gazelle.asset`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `cpu` | `string` | yes |  |
| `name` | `name` | yes |  |
| `os` | `string` | yes |  |
| `sha256` | `string` | yes |  |
| `url` | `string` | yes |  |

Tag `dx_gazelle.from_source`:

No attributes.

Tag `dx_gazelle.language`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `library` | `label` | yes |  |

#### `dx_licenses`

Module extension.

Tag `dx_licenses.accept`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `ids` | `string_list` | yes |  |
| `terms_sha256` | `string_dict` | no | `{}` |

#### `dx_pins`

Module extension.

Tag `dx_pins.archive`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `build` | `label` | no | `None` |
| `bzl_srcs` | `boolean` | no | `False` |
| `crlf` | `string_list` | no | `[]` |
| `dropped` | `string_list` | no | `[]` |
| `files` | `label_dict_unary` | no | `{}` |
| `key` | `string` | no | `""` |
| `name` | `name` | yes |  |
| `patches` | `label_list` | no | `[]` |
| `pins` | `label` | yes |  |
| `required` | `string_list` | no | `[]` |

Tag `dx_pins.data`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `name` | `name` | yes |  |
| `pins` | `label` | yes |  |

Tag `dx_pins.pin`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `aliases` | `string_dict` | no | `{}` |
| `build` | `label` | no | `None` |
| `compatible_with` | `string` | no | `"exec"` |
| `exec_aliases` | `string_list` | no | `[]` |
| `name` | `name` | yes |  |
| `pins` | `label` | yes |  |
| `toolchain_type` | `label` | no | `None` |

#### `dx_prebuilt`

Module extension.

Tag `dx_prebuilt.asset`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `cpu` | `string` | yes |  |
| `name` | `name` | yes |  |
| `os` | `string` | yes |  |
| `sha256` | `string` | yes |  |
| `url` | `string` | yes |  |

Tag `dx_prebuilt.from_source`:

No attributes.

#### `dx_toolchains`

Module extension.

Tag `dx_toolchains.package`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `build` | `label` | yes |  |
| `name` | `name` | yes |  |

### `@rules_dx//csharp:defs.bzl`

#### `csharp_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `deps` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `csharp_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `upstream_visibility` | no | `None` |
| `kwargs` | no |  |

#### `csharp_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `library` | yes |  |
| `package` | no | `""` |
| `kwargs` | no |  |

#### `csharp_fuzz_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `deps` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `csharp_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `csharp_proto_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `proto` | yes |  |
| `deps` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `csharp_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//csharp:extensions.bzl`

#### `nuget`

Module extension.

Tag `nuget.lock`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `lock` | `label` | yes |  |
| `name` | `name` | yes |  |

### `@rules_dx//cuda:defs.bzl`

#### `cuda_component_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `component` | `label` | yes |  |
| `component_name` | `string` | yes |  |
| `name` | `name` | yes |  |

#### `cuda_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `gpu_archs` | yes |  |
| `deps` | no | `None` |
| `hdrs` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `target_compatible_with` | no | `None` |
| `rdc` | no | `False` |
| `cuda_copts` | no | `None` |
| `kwargs` | no |  |

#### `cuda_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `lib` | yes |  |
| `package` | no | `""` |
| `kwargs` | no |  |

#### `cuda_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `gpu_archs` | yes |  |
| `hdrs` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `target_compatible_with` | no | `None` |
| `rdc` | no | `False` |
| `kwargs` | no |  |

#### `cuda_objects`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `gpu_archs` | yes |  |
| `hdrs` | no | `None` |
| `deps` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `target_compatible_with` | no | `None` |
| `kwargs` | no |  |

#### `cuda_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `gpu_archs` | yes |  |
| `deps` | no | `None` |
| `hdrs` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `target_compatible_with` | no | `None` |
| `rdc` | no | `False` |
| `min_compute_capability` | no | `None` |
| `min_driver` | no | `None` |
| `env` | no | `None` |
| `data` | no | `None` |
| `cuda_copts` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//cuda:extensions.bzl`

#### `cuda_component_repo`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `license_error` | yes |  |
| `kwargs` | no |  |

#### `cuda_component_repo_name`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `component` | yes |  |
| `platform` | yes |  |
| `version` | yes |  |

#### `cuda_license_error`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `ids` | yes |  |

#### `cuda`

Module extension.

No tags.

### `@rules_dx//d:defs.bzl`

#### `d_doc_shard`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `config` | `string` | no | `""` |
| `lib` | `label` | yes |  |
| `name` | `name` | yes |  |
| `package` | `string` | no | `""` |

#### `d_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `d_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `d_fuzz_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `dopts` | no | `None` |
| `linkopts` | no | `None` |
| `kwargs` | no |  |

#### `d_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `d_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//d:extensions.bzl`

#### `d`

Module extension.

No tags.

#### `dub`

Module extension.

Tag `dub.lock`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `lock` | `label` | yes |  |
| `name` | `name` | yes |  |

### `@rules_dx//dart:defs.bzl`

#### `dart_codegen`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `asset_deps` | `label_list` | no | `[]` |
| `config` | `string` | no | `""` |
| `data` | `label_list` | no | `[]` |
| `deps` | `label_list` | no | `[]` |
| `generator` | `label` | yes |  |
| `generator_args` | `string_list` | no | `[]` |
| `name` | `name` | yes |  |
| `output_suffixes` | `string_list` | yes |  |
| `package` | `label` | no | `None` |
| `package_config` | `label` | no | `None` |
| `parts` | `label_list` | no | `[]` |
| `src` | `label` | yes |  |

#### `dart_config`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_generated` | `boolean` | no | `False` |
| `data` | `label_list` | no | `[]` |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |

#### `dart_doc_shard`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `lib` | `label` | yes |  |
| `name` | `name` | yes |  |
| `package` | `string` | no | `""` |

#### `dart_package_metadata`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `build_hook` | `label` | no | `None` |
| `code_assets` | `label_list` | no | `[]` |
| `data_assets` | `label_list` | no | `[]` |
| `language_version` | `string` | no | `""` |
| `license_file` | `label` | no | `None` |
| `link_hook` | `label` | no | `None` |
| `link_hook_assets` | `string_list` | no | `[]` |
| `linked_code_assets` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `package_name` | `string` | yes |  |
| `version` | `string` | no | `""` |

#### `dart_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `deps` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `dart_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `dart_js_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `dart_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `dart_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `dart_wasm_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//dart:extensions.bzl`

#### `pub`

Module extension.

Tag `pub.cycle`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `packages` | `string_list` | yes |  |

Tag `pub.from_lock`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `build_from_pubspec` | `string_list` | no | `[]` |
| `ignore_hooks` | `string_list` | no | `[]` |
| `ignored` | `string_dict` | no | `{}` |
| `lock` | `label` | yes |  |
| `name` | `name` | yes |  |
| `resolve` | `label_dict_unary` | no | `{}` |

Tag `pub.package`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `ignore_hook` | `boolean` | no | `False` |
| `name` | `name` | yes |  |
| `sha256` | `string` | yes |  |
| `version` | `string` | yes |  |

Tag `pub.settings`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `check_direct_dependencies` | `string` | no | `"warning"` |

### `@rules_dx//deploy:defs.bzl`

#### `deployment`

Rule. Run it with `bazel run`.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `app` | `label` | no | `None` |
| `artifacts` | `label_list` | no | `[]` |
| `build_mode` | `string` | no | `"release"` |
| `config` | `label` | no | `None` |
| `data` | `label_list` | no | `[]` |
| `deploy` | `label` | yes |  |
| `name` | `name` | yes |  |
| `sbom` | `label` | no | `None` |
| `size_budget` | `string` | no | `""` |

#### `native_runtime`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `name` | `name` | yes |  |
| `notice` | `label` | no | `None` |
| `provides` | `string_list` | no | `[]` |
| `redistribution` | `string` | yes |  |
| `source` | `string` | yes |  |
| `srcs` | `label_list` | no | `[]` |
| `version` | `string` | yes |  |

#### `oci_image_quality`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `build_files` | `label_list` | yes |  |
| `image` | `label` | yes |  |
| `name` | `name` | yes |  |
| `structure_tests` | `label_list` | no | `[]` |

#### `resources`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `mappings` | `label_string_dict` | no | `{}` |
| `name` | `name` | yes |  |
| `processor` | `label` | no | `None` |
| `processor_args` | `string_list` | no | `[]` |

#### `sbom`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `artifact` | `label` | yes |  |
| `go_mod` | `label` | no | `None` |
| `maven_lock` | `label` | no | `None` |
| `name` | `name` | yes |  |
| `npm_lock` | `label` | no | `None` |
| `package_name` | `string` | no | `""` |
| `pip_lock` | `label` | no | `None` |
| `supplier` | `string` | no | `"rules_dx"` |

#### `archive_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `app` | yes |  |
| `build_mode` | no | `"release"` |
| `size_budget` | no | `None` |
| `sbom` | no | `None` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `None` |

#### `cloudflare_pages_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `account_id` | yes |  |
| `project` | yes |  |
| `branch` | yes |  |
| `production_branch` | yes |  |
| `site` | yes |  |
| `wrangler` | no | `None` |
| `build_mode` | no | `"release"` |
| `sbom` | no | `None` |
| `size_budget` | no | `None` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `None` |

#### `conan_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `recipe` | yes |  |
| `remote` | yes |  |
| `remote_url` | yes |  |
| `package` | no | `""` |
| `version` | no | `""` |
| `conan` | no | `None` |
| `conan_files` | no | `None` |
| `build_mode` | no | `"release"` |
| `sbom` | no | `None` |
| `size_budget` | no | `None` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `None` |

#### `consumer_archive`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `app` | yes |  |
| `min_runtime` | yes |  |
| `linkage` | no | `"dynamic"` |
| `resources` | no | `[]` |
| `runtime` | no | `[]` |
| `notice` | no | `None` |
| `license_file` | no | `None` |
| `check_imports` | no | `True` |
| `format` | no | `"tar.gz"` |
| `platform` | no | `None` |
| `tags` | no | `None` |
| `target_compatible_with` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `None` |

#### `cpan_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `dist` | yes |  |
| `package` | no | `""` |
| `version` | no | `""` |
| `build_mode` | no | `"release"` |
| `sbom` | no | `None` |
| `size_budget` | no | `None` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `None` |

#### `crates_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `crate` | yes |  |
| `package` | no | `""` |
| `version` | no | `""` |
| `build_mode` | no | `"release"` |
| `api_compat` | no | `"fail"` |
| `api_baseline` | no | `"registry"` |
| `sbom` | no | `None` |
| `size_budget` | no | `None` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `None` |

#### `dsym`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `binary` | yes |  |
| `kwargs` | no |  |

#### `github_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `artifacts` | yes |  |
| `release_tag` | no | `""` |
| `tag_from_head` | no | `False` |
| `build_mode` | no | `"release"` |
| `sbom` | no | `None` |
| `notes` | no | `"commits"` |
| `notes_file` | no | `None` |
| `notes_diff` | no | `[]` |
| `api_compat` | no | `"fail"` |
| `api_baseline` | no | `"registry"` |
| `go_modules` | no | `[]` |
| `sign` | no | `"auto"` |
| `registry_deploys` | no | `[]` |
| `attach` | no | `True` |
| `size_budget` | no | `None` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `None` |

#### `hackage_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `sdist` | yes |  |
| `package` | no | `""` |
| `version` | no | `""` |
| `build_mode` | no | `"release"` |
| `sbom` | no | `None` |
| `size_budget` | no | `None` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `None` |

#### `helm_chart_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `chart` | yes |  |
| `registry` | yes |  |
| `version` | no | `""` |
| `repository` | no | `""` |
| `build_mode` | no | `"release"` |
| `sbom` | no | `None` |
| `size_budget` | no | `None` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `None` |

#### `hex_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `package` | no | `""` |
| `version` | no | `""` |
| `tool` | no | `"mix"` |
| `build_mode` | no | `"release"` |
| `sbom` | no | `None` |
| `size_budget` | no | `None` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `None` |

#### `homebrew_tap_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `release` | yes |  |
| `repo` | yes |  |
| `tap` | yes |  |
| `formula` | no | `None` |
| `binary` | no | `None` |
| `desc` | no | `None` |
| `homepage` | no | `None` |
| `build_mode` | no | `"release"` |
| `sbom` | no | `None` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `None` |

#### `linux_package_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `binary` | yes |  |
| `version` | yes |  |
| `package` | no | `""` |
| `formats` | no | `["deb", "rpm"]` |
| `platform` | no | `None` |
| `maintainer` | no | `"unknown <unknown@example.com>"` |
| `description` | no | `""` |
| `install_path` | no | `None` |
| `extra_files` | no | `{}` |
| `release_tag` | no | `""` |
| `tag_from_head` | no | `False` |
| `notes` | no | `"commits"` |
| `notes_file` | no | `None` |
| `build_mode` | no | `"release"` |
| `sbom` | no | `None` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `None` |

#### `luarocks_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `rockspec` | yes |  |
| `package` | no | `""` |
| `version` | no | `""` |
| `build_mode` | no | `"release"` |
| `sbom` | no | `None` |
| `size_budget` | no | `None` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `None` |

#### `maven_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `jar` | yes |  |
| `pom` | yes |  |
| `group` | yes |  |
| `artifact` | no | `""` |
| `version` | no | `""` |
| `build_mode` | no | `"release"` |
| `sources` | no | `None` |
| `javadoc` | no | `None` |
| `sbom` | no | `None` |
| `api_compat` | no | `"fail"` |
| `api_baseline` | no | `"registry"` |
| `size_budget` | no | `None` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `None` |

#### `npm_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `package` | no | `""` |
| `dist_tag` | no | `"latest"` |
| `srcs` | no | `None` |
| `registry_url` | no | `"https://registry.npmjs.org"` |
| `access` | no | `"public"` |
| `provenance` | no | `""` |
| `build_mode` | no | `"release"` |
| `sbom` | no | `None` |
| `api_compat` | no | `"fail"` |
| `api_baseline` | no | `"registry"` |
| `size_budget` | no | `None` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `None` |

#### `nuget_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `nupkg` | yes |  |
| `package` | no | `""` |
| `version` | no | `""` |
| `registry_url` | no | `"https://api.nuget.org/v3/index.json"` |
| `build_mode` | no | `"release"` |
| `sbom` | no | `None` |
| `api_compat` | no | `"fail"` |
| `api_baseline` | no | `"registry"` |
| `size_budget` | no | `None` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `None` |

#### `oci_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `image_tar` | yes |  |
| `platform` | yes |  |
| `image_tag` | no | `"latest"` |
| `registry` | no | `"ghcr.io"` |
| `repository` | no | `""` |
| `build_mode` | no | `"release"` |
| `size_budget` | no | `None` |
| `sbom` | no | `None` |
| `scan` | no | `"fail"` |
| `scan_severity` | no | `"HIGH"` |
| `sign` | no | `"auto"` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `None` |

#### `oci_image_from_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `binary` | yes |  |
| `base` | no | `"static"` |
| `platforms` | no | `["@rules_dx//deploy:linux_amd64"]` |
| `tags` | no | `None` |
| `main` | no | `None` |
| `main_class` | no | `None` |
| `runtime` | no | `False` |
| `structure_tests` | no | `[]` |
| `visibility` | no | `None` |
| `testonly` | no | `None` |

#### `octopus_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `project` | yes |  |
| `package` | no | `""` |
| `channel` | no | `"Default"` |
| `version` | no | `""` |
| `deploy_to` | no | `[]` |
| `space` | no | `""` |
| `registry_url` | no | `"https://octopus.example.invalid"` |
| `build_mode` | no | `"release"` |
| `sbom` | no | `None` |
| `size_budget` | no | `None` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `None` |

#### `offline_bundle`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `advisory` | yes |  |
| `launchers` | yes |  |
| `advisory_meta` | yes |  |

#### `opam_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `package` | yes |  |
| `version` | yes |  |
| `opam_file` | yes |  |
| `build_mode` | no | `"release"` |
| `sbom` | no | `None` |
| `size_budget` | no | `""` |
| `tags` | no | `[]` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `[]` |

#### `packagist_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `package` | yes |  |
| `composer_json` | yes |  |
| `repository_url` | yes |  |
| `build_mode` | no | `"release"` |
| `sbom` | no | `None` |
| `size_budget` | no | `""` |
| `tags` | no | `[]` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `[]` |

#### `pdb`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `binary` | yes |  |
| `kwargs` | no |  |

#### `pub_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `package` | no | `""` |
| `version` | no | `""` |
| `registry_url` | no | `"https://pub.dev"` |
| `build_mode` | no | `"release"` |
| `sbom` | no | `None` |
| `api_compat` | no | `"fail"` |
| `api_baseline` | no | `"registry"` |
| `size_budget` | no | `None` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `None` |

#### `pypi_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `wheel` | yes |  |
| `sdist` | no | `None` |
| `registry_url` | no | `"https://upload.pypi.org/legacy/"` |
| `build_mode` | no | `"release"` |
| `sbom` | no | `None` |
| `api_compat` | no | `"fail"` |
| `api_baseline` | no | `"registry"` |
| `api_package` | no | `""` |
| `api_src` | no | `""` |
| `size_budget` | no | `None` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `None` |

#### `qt_bundle_runtime`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `bundle` | yes |  |
| `qt` | no | `None` |
| `kwargs` | no |  |

#### `qt_consumer_archive`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `app` | yes |  |
| `min_runtime` | no | `None` |
| `runtime` | no | `[]` |
| `kwargs` | no |  |

#### `rubygems_deploy`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `gem` | yes |  |
| `package` | no | `""` |
| `version` | no | `""` |
| `ruby` | no | `None` |
| `ruby_files` | no | `None` |
| `build_mode` | no | `"release"` |
| `sbom` | no | `None` |
| `size_budget` | no | `None` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `target_compatible_with` | no | `None` |

#### `split_debug_info`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `binary` | yes |  |
| `kwargs` | no |  |

#### `DeployInfo`

Provider.

Fields: `app`, `artifacts`, `build_mode`, `sbom`, `size_budget`, `size_stamp`.

#### `NativeRuntimeInfo`

Provider.

Fields: `files`, `name`, `notice`, `provides`, `redistribution`, `source`, `version`.

### `@rules_dx//dockerfile:defs.bzl`

#### `dockerfile_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

### `@rules_dx//dotnet:defs.bzl`

### `@rules_dx//dotnet:extensions.bzl`

#### `dotnet`

Module extension.

Tag `dotnet.bootstrap_toolchain`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `dotnet_version` | `string` | yes |  |

Tag `dotnet.toolchain`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `dotnet_version` | `string` | no | `""` |
| `global_json` | `label` | no | `None` |

### `@rules_dx//elixir:defs.bzl`

#### `elixir_app`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `kwargs` | no |  |

#### `elixir_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `tags` | no | `None` |
| `deps` | no | `None` |
| `kwargs` | no |  |

#### `elixir_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `app` | yes |  |
| `package` | no | `""` |
| `kwargs` | no |  |

#### `elixir_escript_archive`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `elixir_escript_headers`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `module` | yes |  |

#### `elixir_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//elixir:extensions.bzl`

#### `elixir`

Module extension.

No tags.

#### `hex`

Module extension.

Tag `hex.lock`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `lock` | `label` | yes |  |
| `name` | `name` | yes |  |

### `@rules_dx//erlang:defs.bzl`

#### `erlang_release`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `apps` | `label_list` | yes |  |
| `name` | `name` | yes |  |

#### `erlang_app`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | no | `"erlang_app"` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `kwargs` | no |  |

#### `erlang_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `suite_name` | no | `""` |
| `tags` | no | `None` |
| `runtime_deps` | no | `None` |
| `kwargs` | no |  |

#### `erlang_bytecode`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `erlang_ct_suite_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `suite_name` | no | `""` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `erlang_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `app` | yes |  |
| `package` | no | `""` |
| `kwargs` | no |  |

#### `erlang_erlc_opts`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `kwargs` | no |  |

#### `erlang_escript_archive`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `erlang_eunit_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | no | `"eunit"` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `erlang_nif`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `hdrs` | no | `[]` |
| `deps` | no | `None` |
| `copts` | no | `[]` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `erlang_test_app`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | no | `"test_erlang_app"` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `erlang_xrl_yrl`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `kwargs` | no |  |

#### `ErlangAppInfo`

Provider.

Fields: `app_name`, `beam`, `deps`, `extra_apps`, `include`, `license_files`, `priv`, `srcs`.

### `@rules_dx//erlang:extensions.bzl`

#### `erlang`

Module extension.

No tags.

### `@rules_dx//flatbuffers:defs.bzl`

#### `flatbuffers_cc_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `codegen_path` | `string` | no | `""` |
| `deps` | `label_list` | no | `[]` |
| `gen_object_api` | `boolean` | no | `False` |
| `name` | `name` | yes |  |

#### `flatbuffers_java_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `codegen_path` | `string` | no | `""` |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |

#### `flatbuffers_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

#### `flatbuffers_conform_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `library` | yes |  |
| `baseline` | yes |  |
| `root` | no | `None` |
| `baseline_root` | no | `None` |
| `kwargs` | no |  |

#### `flatbuffers_rust_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `library` | yes |  |
| `crate_name` | no | `None` |
| `deps` | no | `None` |
| `root` | no | `None` |
| `codegen_path` | no | `None` |
| `kwargs` | no |  |

#### `FlatBuffersInfo`

Provider.

Fields: `direct_srcs`, `direct_staged`, `stage_root`, `stage_roots`, `staged`, `transitive_srcs`.

### `@rules_dx//flutter:defs.bzl`

#### `flutter_font`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `asset` | `string` | no | `""` |
| `name` | `name` | yes |  |
| `src` | `label` | yes |  |
| `style` | `string` | no | `""` |
| `weight` | `int` | no | `0` |

#### `flutter_font_family`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `family` | `string` | no | `""` |
| `fonts` | `label_list` | yes |  |
| `name` | `name` | yes |  |

#### `flutter_gen_l10n`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `arbs` | `label_list` | yes |  |
| `header` | `string` | no | `""` |
| `name` | `name` | yes |  |
| `nullable_getter` | `boolean` | no | `True` |
| `output_class` | `string` | no | `"AppLocalizations"` |
| `output_dir` | `string` | no | `""` |
| `output_localization_file` | `string` | no | `"app_localizations.dart"` |
| `preferred_supported_locales` | `string_list` | no | `[]` |
| `relax_syntax` | `boolean` | no | `False` |
| `required_resource_attributes` | `boolean` | no | `False` |
| `suppress_warnings` | `boolean` | no | `False` |
| `template_arb_file` | `string` | no | `"app_en.arb"` |
| `use_deferred_loading` | `boolean` | no | `False` |
| `use_escaping` | `boolean` | no | `False` |
| `use_named_parameters` | `boolean` | no | `False` |

#### `flutter_android_app`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `application` | yes |  |
| `application_id` | yes |  |
| `android_abi` | no | `"arm64"` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `flutter_application`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `flutter_ios_app`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `application` | yes |  |
| `bundle_id` | yes |  |
| `kwargs` | no |  |

#### `flutter_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `flutter_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `flutter_web_app`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `application` | yes |  |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `flutter_web_bundle`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `application` | yes |  |
| `kwargs` | no |  |

### `@rules_dx//flutter:extensions.bzl`

#### `flutter`

Module extension.

Tag `flutter.checkout`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `checkout` | `label` | no | `None` |
| `engines_env` | `string` | no | `"FLUTTER_LOCAL_ENGINES"` |
| `material_fonts_sha256` | `string` | no | `""` |
| `name` | `name` | yes |  |
| `path_env` | `string` | no | `"FLUTTER_CHECKOUT"` |

Tag `flutter.custom_engine`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `engine` | `label` | yes |  |
| `flutter_version` | `string` | yes |  |
| `hosts` | `string_list` | yes |  |
| `name` | `name` | yes |  |

Tag `flutter.custom_framework`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `flutter_version` | `string` | yes |  |
| `framework` | `label` | yes |  |
| `material_fonts_sha256` | `string` | no | `""` |
| `name` | `name` | yes |  |

Tag `flutter.custom_sdk`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `engine` | `label` | yes |  |
| `framework` | `label` | yes |  |
| `hosts` | `string_list` | yes |  |
| `material_fonts_sha256` | `string` | no | `""` |
| `name` | `name` | yes |  |

Tag `flutter.toolchain`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allow_upgrade_by_dependencies` | `boolean` | no | `False` |
| `flutter_version` | `string` | no | `""` |
| `name` | `name` | no | `"flutter"` |

Tag `flutter.use_sdk`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `name` | `name` | yes |  |

#### `pub`

Module extension.

Tag `pub.annotation`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `android_deps` | `label_list` | no | `[]` |
| `build_config_fields` | `string_dict` | no | `{}` |
| `package` | `string` | yes |  |

Tag `pub.cycle`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `packages` | `string_list` | yes |  |

Tag `pub.from_lock`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `build_from_pubspec` | `string_list` | no | `[]` |
| `ignore_hooks` | `string_list` | no | `[]` |
| `ignored` | `string_dict` | no | `{}` |
| `lock` | `label` | yes |  |
| `name` | `name` | yes |  |
| `resolve` | `label_dict_unary` | no | `{}` |
| `swift_package_resolved` | `label` | no | `None` |

Tag `pub.plugin_overlays`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `roots` | `label_list` | no | `[]` |

Tag `pub.settings`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `check_direct_dependencies` | `string` | no | `"warning"` |

### `@rules_dx//fortran:defs.bzl`

#### `fortran_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `fortran_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `lib` | yes |  |
| `package` | no | `""` |
| `kwargs` | no |  |

#### `fortran_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `fortran_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//fortran:extensions.bzl`

#### `fortran`

Module extension.

No tags.

#### `fpm`

Module extension.

Tag `fpm.lock`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `lock` | `label` | yes |  |
| `name` | `name` | yes |  |

### `@rules_dx//fsharp:defs.bzl`

#### `fsharp_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `deps` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `fsharp_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `upstream_visibility` | no | `None` |
| `kwargs` | no |  |

#### `fsharp_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `library` | yes |  |
| `package` | no | `""` |
| `kwargs` | no |  |

#### `fsharp_fuzz_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `deps` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `fsharp_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `fsharp_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//go:defs.bzl`

#### `go_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `go_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `go_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `library` | yes |  |
| `package` | no | `""` |
| `goos` | no | `"linux"` |
| `goarch` | no | `"amd64"` |
| `build_tags` | no | `[]` |
| `kwargs` | no |  |

#### `go_fuzz_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `target` | yes |  |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `go_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `importpath` | yes |  |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `go_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//go:extensions.bzl`

#### `go`

Module extension.

Tag `go.archive_override`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `patch_cmds` | `string_list` | no | `[]` |
| `patch_strip` | `int` | no | `0` |
| `patches` | `label_list` | no | `[]` |
| `path` | `string` | yes |  |
| `sha256` | `string` | no | `""` |
| `strip_prefix` | `string` | no | `""` |
| `urls` | `string_list` | no | `[]` |

Tag `go.config`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `check_direct_dependencies` | `string` | no | `""` |
| `debug_mode` | `boolean` | no | `False` |
| `go_env` | `string_dict` | no | `{}` |
| `go_env_inherit` | `string_list` | no | `[]` |

Tag `go.from_file`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `fail_on_version_conflict` | `boolean` | no | `True` |
| `go_mod` | `label` | no | `None` |
| `go_work` | `label` | no | `None` |

Tag `go.gazelle_default_attributes`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `build_extra_args` | `string_list` | no | `[]` |
| `build_file_generation` | `string` | no | `"on"` |
| `directives` | `string_list` | no | `[]` |

Tag `go.gazelle_override`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `build_extra_args` | `string_list` | no | `[]` |
| `build_file_generation` | `string` | no | `"on"` |
| `directives` | `string_list` | no | `[]` |
| `path` | `string` | yes |  |

Tag `go.module`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `build_file_proto_mode` | `string` | no | `""` |
| `build_naming_convention` | `string` | no | `""` |
| `indirect` | `boolean` | no | `False` |
| `local_path` | `string` | no | `""` |
| `path` | `string` | yes |  |
| `sum` | `string` | no | `""` |
| `version` | `string` | yes |  |

Tag `go.module_override`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `patch_cmds` | `string_list` | no | `[]` |
| `patch_strip` | `int` | no | `0` |
| `patches` | `label_list` | no | `[]` |
| `path` | `string` | yes |  |
| `repo_name` | `string` | no | `""` |

### `@rules_dx//haskell:defs.bzl`

#### `haskell_doc_shard`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `config` | `string` | no | `""` |
| `lib` | `label` | yes |  |
| `name` | `name` | yes |  |
| `package` | `string` | no | `""` |

#### `haskell_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `haskell_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `haskell_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `testonly` | no | `False` |
| `kwargs` | no |  |

#### `haskell_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//haskell:extensions.bzl`

#### `cabal`

Module extension.

Tag `cabal.lock`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `lock` | `label` | yes |  |
| `name` | `name` | yes |  |

#### `haskell`

Module extension.

No tags.

### `@rules_dx//helm:defs.bzl`

#### `helm_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `name` | `name` | yes |  |
| `namespace` | `string` | no | `"default"` |
| `release_name` | `string` | no | `"release"` |
| `srcs` | `label_list` | no | `[]` |
| `subcharts` | `label_list` | no | `[]` |
| `values` | `label_list` | no | `[]` |

### `@rules_dx//java:defs.bzl`

#### `java_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `deps` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `java_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `java_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `lib` | yes |  |
| `package` | no | `""` |
| `deps` | no | `[]` |
| `kwargs` | no |  |

#### `java_fuzz_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `target_class` | no | `None` |
| `deps` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `java_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `java_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//java:extensions.bzl`

#### `maven`

Module extension.

Tag `maven.amend_artifact`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `coordinates` | `string` | yes |  |
| `exclusions` | `string_list` | no | `[]` |
| `force_version` | `string` | no | `""` |
| `name` | `name` | no | `"maven"` |
| `neverlink` | `string` | no | `""` |
| `testonly` | `string` | no | `""` |

Tag `maven.artifact`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `artifact` | `string` | yes |  |
| `classifier` | `string` | no | `""` |
| `exclusions` | `string_list` | no | `[]` |
| `force_version` | `boolean` | no | `False` |
| `group` | `string` | yes |  |
| `name` | `name` | no | `"maven"` |
| `neverlink` | `boolean` | no | `False` |
| `packaging` | `string` | no | `""` |
| `testonly` | `boolean` | no | `False` |
| `version` | `string` | no | `""` |

Tag `maven.from_toml`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `bom_modules` | `string_list` | no | `[]` |
| `libs_versions_toml` | `label` | yes |  |
| `name` | `name` | no | `"maven"` |

Tag `maven.install`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `aar_import_bzl_label` | `string` | no | `"@rules_android//rules:rules.bzl"` |
| `additional_coursier_options` | `string_list` | no | `[]` |
| `additional_netrc_lines` | `string_list` | no | `[]` |
| `artifacts` | `string_list` | no | `[]` |
| `boms` | `string_list` | no | `[]` |
| `duplicate_version_warning` | `string` | no | `"warn"` |
| `excluded_artifacts` | `string_list` | no | `[]` |
| `exclusions` | `string_list` | no | `[]` |
| `fail_if_repin_required` | `boolean` | no | `True` |
| `fail_on_missing_checksum` | `boolean` | no | `True` |
| `fetch_javadoc` | `boolean` | no | `False` |
| `fetch_sources` | `boolean` | no | `False` |
| `generate_compat_repositories` | `boolean` | no | `False` |
| `ignore_empty_files` | `boolean` | no | `False` |
| `index_file` | `label` | no | `None` |
| `known_contributing_modules` | `string_list` | no | `[]` |
| `lock_file` | `label` | no | `None` |
| `name` | `name` | no | `"maven"` |
| `repin_instructions` | `string` | no | `""` |
| `repositories` | `string_list` | no | `["https://repo1.maven.org/maven2"]` |
| `resolve_timeout` | `int` | no | `600` |
| `resolver` | `string` | no | `"coursier"` |
| `resolver_extra_dependencies` | `label_list` | no | `[]` |
| `strict_visibility` | `boolean` | no | `False` |
| `strict_visibility_value` | `label_list` | no | `["@rules_dx//visibility:private"]` |
| `use_credentials_from_home_netrc_file` | `boolean` | no | `False` |
| `use_starlark_android_rules` | `boolean` | no | `False` |
| `version_conflict_policy` | `string` | no | `"default"` |

Tag `maven.override`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `coordinates` | `string` | yes |  |
| `name` | `name` | no | `"maven"` |
| `target` | `label` | yes |  |
| `visibility` | `string_list` | no | `[]` |

### `@rules_dx//javascript:defs.bzl`

#### `javascript_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `main` | no | `None` |
| `node_modules` | yes |  |
| `data` | no | `None` |
| `args` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `javascript_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `javascript_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `tsconfig` | yes |  |
| `entry_points` | yes |  |
| `package` | yes |  |
| `deps` | no | `[]` |
| `options` | no | `[]` |
| `kwargs` | no |  |

#### `javascript_fuzz_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `main` | no | `None` |
| `node_modules` | yes |  |
| `data` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `javascript_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `javascript_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `runner` | no | `"jest"` |
| `node_modules` | no | `None` |
| `data` | no | `None` |
| `package_json` | no | `None` |
| `visibility` | no | `None` |
| `env_inherit` | no | `None` |
| `kwargs` | no |  |

#### `javascript_wasm_bindgen_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `bindgen` | yes |  |
| `deps` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `javascript_web_app`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `html` | yes |  |
| `bindgen` | yes |  |
| `assets` | no | `None` |
| `threads` | no | `False` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `javascript_web_bundle`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `app` | yes |  |
| `bundler` | yes |  |
| `config` | no | `None` |
| `entry` | no | `None` |
| `bundler_args` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//javascript:extensions.bzl`

#### `javascript_dev_hub`

Module extension.

Tag `javascript_dev_hub.link`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `hub` | `string` | yes |  |
| `name` | `name` | yes |  |

#### `npm`

Module extension.

Tag `npm.npm_exclude_package_contents`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `package` | `string` | yes |  |
| `patterns` | `string_list` | no | `[]` |
| `presets` | `string_list` | no | `["basic"]` |

Tag `npm.npm_import`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `bins` | `string_dict` | no | `{}` |
| `commit` | `string` | no | `""` |
| `custom_postinstall` | `string` | no | `""` |
| `deps` | `string_dict` | no | `{}` |
| `exclude_package_contents` | `string_list` | no | `[]` |
| `exclude_package_contents_presets` | `string_list` | no | `["basic"]` |
| `extra_build_content` | `string` | no | `""` |
| `extract_full_archive` | `boolean` | no | `False` |
| `integrity` | `string` | no | `""` |
| `lifecycle_build_target` | `boolean` | no | `False` |
| `lifecycle_hooks` | `string_list` | no | `[]` |
| `lifecycle_hooks_env` | `string_list` | no | `[]` |
| `lifecycle_hooks_execution_requirements` | `string_list` | no | `["no-sandbox"]` |
| `lifecycle_hooks_use_default_shell_env` | `boolean` | no | `False` |
| `name` | `name` | no | `""` |
| `npm_auth` | `string` | no | `""` |
| `npm_auth_basic` | `string` | no | `""` |
| `npm_auth_password` | `string` | no | `""` |
| `npm_auth_username` | `string` | no | `""` |
| `package` | `string` | yes |  |
| `package_visibility` | `string_list` | no | `["//visibility:public"]` |
| `patch_args` | `string_list` | no | `["-p0"]` |
| `patch_tool` | `label` | no | `None` |
| `patches` | `label_list` | no | `[]` |
| `replace_package` | `string` | no | `""` |
| `root_package` | `string` | no | `""` |
| `url` | `string` | no | `""` |
| `version` | `string` | yes |  |

Tag `npm.npm_replace_package`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `package` | `string` | yes |  |
| `replacement` | `label` | yes |  |

Tag `npm.npm_translate_lock`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `bins` | `string_list_dict` | no | `{}` |
| `credential_helpers` | `label_string_dict` | no | `{}` |
| `custom_postinstalls` | `string_dict` | no | `{}` |
| `data` | `label_list` | no | `[]` |
| `external_repository_action_cache` | `string` | no | `".aspect/rules/external_repository_action_cache"` |
| `generate_bzl_library_targets` | `boolean` | no | `False` |
| `lifecycle_hooks` | `string_list_dict` | no | `{}` |
| `lifecycle_hooks_envs` | `string_list_dict` | no | `{}` |
| `lifecycle_hooks_exclude` | `string_list` | no | `[]` |
| `lifecycle_hooks_execution_requirements` | `string_list_dict` | no | `{}` |
| `lifecycle_hooks_no_sandbox` | `boolean` | no | `True` |
| `lifecycle_hooks_use_default_shell_env` | `string_dict` | no | `{}` |
| `name` | `name` | no | `""` |
| `no_dev` | `boolean` | no | `False` |
| `no_optional` | `boolean` | no | `False` |
| `node_toolchain_prefix` | `string` | no | `"nodejs"` |
| `npm_package_lock` | `label` | no | `None` |
| `npm_package_target_name` | `string` | no | `"pkg"` |
| `npmrc` | `label` | no | `None` |
| `package_visibility` | `string_list_dict` | no | `{}` |
| `patch_args` | `string_list_dict` | no | `{"*": ["-p0"]}` |
| `patch_tool` | `label` | no | `None` |
| `patches` | `string_list_dict` | no | `{}` |
| `pnpm_lock` | `label` | no | `None` |
| `preupdate` | `label_list` | no | `[]` |
| `public_hoist_packages` | `string_list_dict` | no | `{}` |
| `quiet` | `boolean` | no | `True` |
| `run_lifecycle_hooks` | `boolean` | no | `True` |
| `update_pnpm_lock` | `boolean` | no | `False` |
| `use_home_npmrc` | `boolean` | no | `False` |
| `use_pnpm` | `label` | no | `"@pnpm//:package/bin/pnpm.cjs"` |
| `verify_node_modules_ignored` | `label` | no | `None` |
| `verify_patches` | `label` | no | `None` |
| `yarn_lock` | `label` | no | `None` |

#### `pnpm`

Module extension.

Tag `pnpm.pnpm`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `include_npm` | `boolean` | no | `False` |
| `name` | `name` | no | `"pnpm"` |
| `patch_args` | `string_list` | no | `["-p1"]` |
| `patches` | `label_list` | no | `[]` |
| `pnpm_version` | `string` | no | `""` |
| `pnpm_version_from` | `label` | no | `None` |
| `pnpm_version_integrity` | `string` | no | `""` |

### `@rules_dx//julia:defs.bzl`

#### `julia_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `deps` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `julia_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `julia_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `lib` | yes |  |
| `module` | no | `""` |
| `package` | no | `""` |
| `kwargs` | no |  |

#### `julia_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `julia_standalone_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `kwargs` | no |  |

#### `julia_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//julia:extensions.bzl`

#### `julia`

Module extension.

Tag `julia.from_manifest`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `lockfile` | `label` | no | `None` |
| `manifest` | `label` | no | `None` |
| `manifests` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `pkg_server` | `string` | no | `"https://pkg.julialang.org"` |

Tag `julia.install`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `lockfile` | `label` | no | `None` |
| `manifest` | `label` | no | `None` |
| `manifests` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `pkg_server` | `string` | no | `"https://pkg.julialang.org"` |

Tag `julia.pkg_annotation`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `dep` | `string` | yes |  |
| `patch_args` | `string_list` | no | `[]` |
| `patch_tool` | `string` | no | `""` |
| `patches` | `label_list` | no | `[]` |

#### `julia_toolchain`

Module extension.

Tag `julia_toolchain.toolchain`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `linker` | `string` | no | `"julia"` |
| `version` | `string` | yes |  |

### `@rules_dx//kotlin:defs.bzl`

#### `kotlin_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `deps` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `kotlin_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `kotlin_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `lib` | yes |  |
| `package` | no | `""` |
| `deps` | no | `[]` |
| `kwargs` | no |  |

#### `kotlin_fuzz_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `target_class` | no | `None` |
| `deps` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `kotlin_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `kotlin_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//kubernetes:defs.bzl`

#### `kubernetes_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

#### `kubernetes_schemas`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `crd_schemas` | `label_string_dict` | no | `{}` |
| `kubernetes_version` | `string` | yes |  |
| `name` | `name` | yes |  |
| `schemas` | `label_list` | no | `[]` |
| `skip_kinds` | `string_list` | no | `[]` |

### `@rules_dx//lua:defs.bzl`

#### `lua_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `deps` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `lua_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `lua_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `lua_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//lua:extensions.bzl`

#### `lua`

Module extension.

No tags.

#### `luarocks`

Module extension.

Tag `luarocks.lock`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `lock` | `label` | yes |  |
| `name` | `name` | yes |  |

### `@rules_dx//make:defs.bzl`

#### `make_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

### `@rules_dx//mdx:defs.bzl`

#### `mdx_build`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `config` | yes |  |
| `node_modules` | yes |  |
| `package_json` | no | `None` |
| `public` | no | `None` |
| `ssr_entry` | no | `None` |
| `deps` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `mdx_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `tsconfig` | yes |  |
| `entry_points` | yes |  |
| `package` | yes |  |
| `deps` | no | `[]` |
| `kwargs` | no |  |

#### `mdx_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `mdx_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `node_modules` | yes |  |
| `data` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//msix:defs.bzl`

#### `msix_package`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `target_compatible_with` | no | `None` |
| `kwargs` | no |  |

#### `msix_sign`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `package` | yes |  |
| `target_compatible_with` | no | `None` |
| `kwargs` | no |  |

#### `MsixInfo`

Provider.

Fields: `arch`, `identity`, `name`, `package`, `publisher`, `version`.

### `@rules_dx//msix:extensions.bzl`

#### `msix`

Module extension.

No tags.

### `@rules_dx//ocaml:defs.bzl`

#### `ocaml_lex`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `name` | `name` | yes |  |
| `opts` | `string_list` | no | `[]` |
| `out` | `output` | yes |  |
| `src` | `label` | no | `None` |

#### `ocaml_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `main` | no | `None` |
| `deps` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `ocaml_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `ocaml_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `lib` | yes |  |
| `package` | no | `""` |
| `config` | no | `"odoc=3.2.2"` |
| `kwargs` | no |  |

#### `ocaml_dune_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `kwargs` | no |  |

#### `ocaml_fuzz_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `main` | no | `None` |
| `deps` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `ocaml_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `ocaml_module`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `struct` | yes |  |
| `sig` | no | `None` |
| `visibility` | no | `None` |
| `instrument_coverage` | no | `True` |
| `coverage_ppx` | no | `False` |
| `kwargs` | no |  |

#### `ocaml_signature`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `src` | yes |  |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `ocaml_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `OCamlCmtInfo`

Provider.

Fields: `owned`.

### `@rules_dx//ocaml:extensions.bzl`

#### `ocaml`

Module extension.

No tags.

#### `opam`

Module extension.

Tag `opam.annotation`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `build_file` | `label` | no | `None` |
| `library` | `string` | no | `""` |
| `package` | `string` | yes |  |
| `strip_prefix` | `string` | no | `""` |

Tag `opam.lock`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `lock` | `label` | yes |  |
| `name` | `name` | yes |  |

### `@rules_dx//perl:defs.bzl`

#### `perl_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `deps` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `perl_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `perl_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `lib` | yes |  |
| `package` | no | `""` |
| `kwargs` | no |  |

#### `perl_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `perl_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//perl:extensions.bzl`

#### `cpan`

Module extension.

Tag `cpan.lock`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `lock` | `label` | yes |  |
| `name` | `name` | yes |  |

#### `perl_pod`

Module extension.

No tags.

### `@rules_dx//php:defs.bzl`

#### `php_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `data` | no | `None` |
| `env` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `php_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `php_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `php_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//php:extensions.bzl`

#### `composer`

Module extension.

Tag `composer.lock`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `lock` | `label` | yes |  |
| `name` | `name` | yes |  |

#### `php`

Module extension.

Tag `php.from_source`:

No attributes.

#### `php_coverage`

Module extension.

No tags.

### `@rules_dx//platforms:defs.bzl`

#### `platform_select`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `branches` | yes |  |

#### `platforms_error`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `platforms` | yes |  |
| `native` | yes |  |

#### `DxPlatformInfo`

Provider.

Fields: `name`, `platforms`.

#### `DxTargetInfo`

Provider.

Fields: `check`, `java_deps`, `link_flags`, `min_os_floor`, `runner`.

### `@rules_dx//platforms:extensions.bzl`

#### `dx_platforms`

Module extension.

Tag `dx_platforms.platform`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `constraint_values` | `label_list` | yes |  |
| `hosts` | `string_list` | no | `[]` |
| `name` | `name` | yes |  |
| `parent` | `label` | no | `None` |

### `@rules_dx//powershell:defs.bzl`

#### `powershell_doc_shard`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `config` | `string` | no | `""` |
| `manifest` | `label` | no | `None` |
| `name` | `name` | yes |  |
| `package` | `string` | no | `""` |
| `srcs` | `label_list` | yes |  |

#### `powershell_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `powershell_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `powershell_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//powershell:extensions.bzl`

#### `powershell`

Module extension.

Tag `powershell.lock`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `lock` | `label` | yes |  |
| `name` | `name` | yes |  |

### `@rules_dx//python:defs.bzl`

#### `python_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `args` | no | `None` |
| `tags` | no | `None` |
| `deps` | no | `None` |
| `kwargs` | no |  |

#### `python_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `python_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `library` | yes |  |
| `package` | no | `""` |
| `docstyle` | no | `"google"` |
| `kwargs` | no |  |

#### `python_fuzz_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `python_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `python_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `runner` | no | `"pytest"` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//python:extensions.bzl`

#### `python`

Module extension.

Tag `python.toolchain`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `python_version` | `string` | yes |  |

#### `uv`

Module extension.

Tag `uv.annotate_packages`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `lock` | `label` | yes |  |
| `src` | `label` | yes |  |

Tag `uv.declare_hub`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `hub_name` | `string` | yes |  |

Tag `uv.override_package`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `config_settings` | `string_list_dict` | no | `{}` |
| `console_scripts` | `string_dict` | no | `{"": ""}` |
| `env` | `string_dict` | no | `{}` |
| `exclude_glob` | `string_list` | no | `[]` |
| `extra_data` | `label_list` | no | `[]` |
| `extra_deps` | `label_list` | no | `[]` |
| `lock` | `label` | no | `None` |
| `monitor_memory` | `boolean` | no | `False` |
| `name` | `name` | yes |  |
| `post_install_patch_strip` | `int` | no | `1` |
| `post_install_patches` | `label_list` | no | `[]` |
| `pre_build_patch_strip` | `int` | no | `1` |
| `pre_build_patches` | `label_list` | no | `[]` |
| `resource_set` | `string` | no | `"default"` |
| `target` | `label` | no | `None` |
| `testonly` | `boolean` | no | `False` |
| `toolchains` | `label_list` | no | `[]` |
| `version` | `string` | no | `""` |

Tag `uv.project`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `default_build_dependencies` | `string_list` | no | `["build"]` |
| `hub_name` | `string` | yes |  |
| `lock` | `label` | yes |  |
| `name` | `name` | no | `""` |
| `pyproject` | `label` | yes |  |
| `version` | `string` | no | `""` |

### `@rules_dx//python:grpc.bzl`

#### `python_grpc_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `proto` | yes |  |
| `py_proto` | yes |  |
| `grpc_tools` | no | `"@pypi//grpcio_tools"` |
| `grpc` | no | `"@pypi//grpcio"` |
| `dep_group` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//qt:defs.bzl`

#### `qt_frameworks`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |

#### `qt_bundle`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `binary` | yes |  |
| `qml_modules` | no | `[]` |
| `licenses` | no | `[]` |
| `display` | no | `[]` |
| `plugins` | no | `[]` |
| `qt` | no | `None` |
| `kwargs` | no |  |

#### `qt_cc_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `hdrs` | no | `[]` |
| `moc_hdrs` | no | `[]` |
| `forms` | no | `[]` |
| `resources` | no | `[]` |
| `resource_prefix` | no | `"/"` |
| `translations` | no | `[]` |
| `deps` | no | `None` |
| `modules` | no | `["core"]` |
| `qt` | no | `None` |
| `plugins` | no | `[]` |
| `copts` | no | `[]` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `qt_cc_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `hdrs` | no | `[]` |
| `moc_hdrs` | no | `[]` |
| `forms` | no | `[]` |
| `resources` | no | `[]` |
| `resource_prefix` | no | `"/"` |
| `translations` | no | `[]` |
| `deps` | no | `None` |
| `modules` | no | `["core"]` |
| `qt` | no | `None` |
| `copts` | no | `[]` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `qt_cc_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `hdrs` | no | `[]` |
| `moc_hdrs` | no | `[]` |
| `forms` | no | `[]` |
| `resources` | no | `[]` |
| `resource_prefix` | no | `"/"` |
| `translations` | no | `[]` |
| `deps` | no | `None` |
| `modules` | no | `["core", "test"]` |
| `qt` | no | `None` |
| `plugins` | no | `[]` |
| `copts` | no | `[]` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `qt_qml_module`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `uri` | yes |  |
| `version` | no | `"1.0"` |
| `qml_srcs` | no | `[]` |
| `qml_depends` | no | `[]` |
| `srcs` | no | `None` |
| `hdrs` | no | `[]` |
| `moc_hdrs` | no | `[]` |
| `resources` | no | `[]` |
| `resource_prefix` | no | `"/"` |
| `translations` | no | `[]` |
| `deps` | no | `None` |
| `modules` | no | `["qml"]` |
| `qt` | no | `None` |
| `copts` | no | `[]` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `qt_rust_cc_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `bridges` | yes |  |
| `rust` | yes |  |
| `cc_deps` | no | `[]` |
| `generator` | no | `None` |
| `uri` | no | `None` |
| `version` | no | `"1.0"` |
| `qml_srcs` | no | `[]` |
| `qml_depends` | no | `[]` |
| `modules` | no | `None` |
| `qt` | no | `None` |
| `copts` | no | `[]` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `qt_rust_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `bridges` | yes |  |
| `srcs` | no | `None` |
| `crate_root` | no | `None` |
| `crate_name` | no | `None` |
| `crate_features` | no | `None` |
| `rustc_flags` | no | `[]` |
| `deps` | no | `None` |
| `cc_deps` | no | `[]` |
| `cxx` | no | pinned |
| `cxx_qt` | no | pinned |
| `generator` | no | `None` |
| `uri` | no | `None` |
| `version` | no | `"1.0"` |
| `qml_srcs` | no | `[]` |
| `qml_depends` | no | `[]` |
| `modules` | no | `None` |
| `qt` | no | `None` |
| `copts` | no | `[]` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//qt:extensions.bzl`

#### `qt`

Module extension.

Tag `qt.cell`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `constraints` | `label_list` | no | `[]` |
| `layout` | `string` | no | `"linux"` |
| `name` | `name` | yes |  |
| `spec` | `label` | yes |  |

Tag `qt.configure`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `mirror` | `string` | no | `""` |
| `runtime_mirror` | `string` | no | `""` |
| `version` | `string` | no | `"6.12.0"` |

Tag `qt.from_source`:

No attributes.

Tag `qt.msvc_runtime`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `notice` | `label` | yes |  |
| `runtime` | `label` | yes |  |

### `@rules_dx//rego:defs.bzl`

#### `rego_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `data` | `label_list` | no | `[]` |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

#### `rego_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//rst:defs.bzl`

#### `rst_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

### `@rules_dx//ruby:defs.bzl`

#### `ruby_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `data` | no | `None` |
| `env` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `ruby_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `ruby_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `lib` | yes |  |
| `package` | no | `""` |
| `deps` | no | `[]` |
| `kwargs` | no |  |

#### `ruby_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `ruby_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//ruby:extensions.bzl`

#### `ruby`

Module extension.

Tag `ruby.lock`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `lock` | `label` | yes |  |
| `name` | `name` | yes |  |

Tag `ruby.toolchain`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `name` | `name` | no | `"ruby"` |

#### `ruby_coverage`

Module extension.

No tags.

### `@rules_dx//rust:defs.bzl`

#### `rust_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `crate` | no | `None` |
| `crate_features` | no | `None` |
| `tags` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `rust_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `crate_name` | no | `None` |
| `edition` | no | `"2024"` |
| `visibility` | no | `None` |
| `wasm` | no | `None` |
| `runtime_dlls` | no | `None` |
| `kwargs` | no |  |

#### `rust_build_script`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `build_script_env` | no | `None` |
| `toolchains` | no | `None` |
| `use_cc_toolchain` | no | `None` |
| `kwargs` | no |  |

#### `rust_docs_site`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `libraries` | yes |  |
| `prose` | yes |  |
| `book_toml` | yes |  |
| `prose_route` | no | `"prose.md"` |
| `source_url` | no | `""` |

#### `rust_fuzz_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `deps` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `rustc_flags` | no | `None` |
| `rustc_env` | no | `None` |
| `libfuzzer_sys` | yes |  |
| `kwargs` | no |  |

#### `rust_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `crate_name` | no | `None` |
| `edition` | no | `"2024"` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `rust_proc_macro`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `crate_name` | no | `None` |
| `edition` | no | `"2024"` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `rust_shared_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `crate_name` | no | `None` |
| `edition` | no | `"2024"` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `rust_static_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `crate_name` | no | `None` |
| `edition` | no | `"2024"` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `rust_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `crate` | no | `None` |
| `edition` | no | `"2024"` |
| `visibility` | no | `None` |
| `wasm` | no | `None` |
| `preopens` | no | `None` |
| `runtime` | no | `None` |
| `coverage_lib` | no | `None` |
| `runtime_dlls` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//rust:extensions.bzl`

#### `cargo`

Module extension.

Tag `cargo.annotation`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `additive_build_file` | `label` | no | `None` |
| `additive_build_file_content` | `string` | no | `""` |
| `alias_rule` | `string` | no | `""` |
| `build_script_compile_data` | `string_list` | no | `[]` |
| `build_script_data` | `string_list` | no | `[]` |
| `build_script_data_glob` | `string_list` | no | `[]` |
| `build_script_deps` | `string_list` | no | `[]` |
| `build_script_env` | `string_dict` | no | `{}` |
| `build_script_env_files` | `string_list` | no | `[]` |
| `build_script_exec_properties` | `string_dict` | no | `{}` |
| `build_script_link_deps` | `string_list` | no | `[]` |
| `build_script_proc_macro_deps` | `string_list` | no | `[]` |
| `build_script_rundir` | `string` | no | `""` |
| `build_script_rustc_env` | `string_dict` | no | `{}` |
| `build_script_toolchains` | `label_list` | no | `[]` |
| `build_script_tools` | `string_list` | no | `[]` |
| `build_script_use_cc_toolchain` | `string` | no | `"auto"` |
| `build_script_use_default_shell_env` | `string` | no | `"auto"` |
| `compile_data` | `string_list` | no | `[]` |
| `compile_data_glob` | `string_list` | no | `[]` |
| `compile_data_glob_excludes` | `string_list` | no | `[]` |
| `crate` | `string` | yes |  |
| `crate_features` | `string_list` | no | `[]` |
| `data` | `string_list` | no | `[]` |
| `data_glob` | `string_list` | no | `[]` |
| `deps` | `string_list` | no | `[]` |
| `disable_pipelining` | `boolean` | no | `False` |
| `extra_aliased_targets` | `string_dict` | no | `{}` |
| `gen_all_binaries` | `boolean` | no | `False` |
| `gen_binaries` | `string_list` | no | `[]` |
| `gen_build_script` | `string` | no | `"auto"` |
| `label_injections` | `label_string_dict` | no | `{}` |
| `link_deps` | `string_list` | no | `[]` |
| `override_target_bin` | `label` | no | `None` |
| `override_target_build_script` | `label` | no | `None` |
| `override_target_lib` | `label` | no | `None` |
| `override_target_proc_macro` | `label` | no | `None` |
| `patch_args` | `string_list` | no | `[]` |
| `patch_tool` | `string` | no | `""` |
| `patches` | `label_list` | no | `[]` |
| `proc_macro_deps` | `string_list` | no | `[]` |
| `repositories` | `string_list` | no | `[]` |
| `rustc_env` | `string_dict` | no | `{}` |
| `rustc_env_files` | `string_list` | no | `[]` |
| `rustc_flags` | `string_list` | no | `[]` |
| `shallow_since` | `string` | no | `""` |
| `version` | `string` | no | `"*"` |

Tag `cargo.annotation_select`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `build_script_compile_data` | `string_list` | no | `[]` |
| `build_script_data` | `string_list` | no | `[]` |
| `build_script_deps` | `string_list` | no | `[]` |
| `build_script_env` | `string_dict` | no | `{}` |
| `build_script_env_files` | `string_list` | no | `[]` |
| `build_script_exec_properties` | `string_dict` | no | `{}` |
| `build_script_link_deps` | `string_list` | no | `[]` |
| `build_script_proc_macro_deps` | `string_list` | no | `[]` |
| `build_script_rundir` | `string` | no | `""` |
| `build_script_rustc_env` | `string_dict` | no | `{}` |
| `build_script_tools` | `string_list` | no | `[]` |
| `compile_data` | `string_list` | no | `[]` |
| `crate` | `string` | yes |  |
| `crate_features` | `string_list` | no | `[]` |
| `data` | `string_list` | no | `[]` |
| `deps` | `string_list` | no | `[]` |
| `link_deps` | `string_list` | no | `[]` |
| `proc_macro_deps` | `string_list` | no | `[]` |
| `repositories` | `string_list` | no | `[]` |
| `rustc_env` | `string_dict` | no | `{}` |
| `rustc_env_files` | `string_list` | no | `[]` |
| `rustc_flags` | `string_list` | no | `[]` |
| `triples` | `string_list` | yes |  |
| `version` | `string` | no | `"*"` |

Tag `cargo.from_cargo`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `cargo_config` | `label` | no | `None` |
| `cargo_lockfile` | `label` | no | `None` |
| `generate_binaries` | `boolean` | no | `False` |
| `generate_build_scripts` | `boolean` | no | `True` |
| `host_tools` | `label` | no | `"@rust_host_tools"` |
| `isolated` | `boolean` | no | `True` |
| `lockfile` | `label` | no | `None` |
| `manifests` | `label_list` | no | `[]` |
| `name` | `name` | no | `"crates"` |
| `skip_cargo_lockfile_overwrite` | `boolean` | no | `False` |
| `strip_internal_dependencies_from_cargo_lockfile` | `boolean` | no | `False` |
| `supported_platform_triples` | `string_list` | no | `["aarch64-apple-darwin", "aarch64-unknown-linux-gnu", "wasm32-unknown-unknown", "wasm32-wasip1", "x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu", "x86_64-unknown-nixos-gnu"]` |

Tag `cargo.from_specs`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `cargo_config` | `label` | no | `None` |
| `cargo_lockfile` | `label` | no | `None` |
| `generate_binaries` | `boolean` | no | `False` |
| `generate_build_scripts` | `boolean` | no | `True` |
| `host_tools` | `label` | no | `"@rust_host_tools"` |
| `isolated` | `boolean` | no | `True` |
| `lockfile` | `label` | no | `None` |
| `name` | `name` | no | `"crates"` |
| `skip_cargo_lockfile_overwrite` | `boolean` | no | `False` |
| `strip_internal_dependencies_from_cargo_lockfile` | `boolean` | no | `False` |
| `supported_platform_triples` | `string_list` | no | `["aarch64-apple-darwin", "aarch64-unknown-linux-gnu", "wasm32-unknown-unknown", "wasm32-wasip1", "x86_64-pc-windows-msvc", "x86_64-unknown-linux-gnu", "x86_64-unknown-nixos-gnu"]` |

Tag `cargo.render_config`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `build_file_template` | `string` | no | `"//:BUILD.{name}-{version}.bazel"` |
| `crate_alias_template` | `string` | no | `"@{repository}//{name}-{version}"` |
| `crate_label_template` | `string` | no | `"@{repository}__{name}-{version}//:{target}"` |
| `crate_repository_template` | `string` | no | `"{repository}__{name}-{version}"` |
| `crates_module_template` | `string` | no | `"//:{file}"` |
| `default_alias_rule_bzl` | `label` | no | `None` |
| `default_alias_rule_name` | `string` | no | `"alias"` |
| `default_package_name` | `string` | no | `""` |
| `generate_cargo_toml_env_vars` | `boolean` | no | `True` |
| `generate_rules_license_metadata` | `boolean` | no | `False` |
| `generate_target_compatible_with` | `boolean` | no | `True` |
| `incompatible_no_root_alias_targets` | `boolean` | no | `False` |
| `platforms_template` | `string` | no | `"@rules_rust//rust/platform:{triple}"` |
| `regen_command` | `string` | no | `""` |
| `repositories` | `string_list` | no | `[]` |
| `vendor_mode` | `string` | no | `""` |

Tag `cargo.spec`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `artifact` | `string` | no | `""` |
| `branch` | `string` | no | `""` |
| `default_features` | `boolean` | no | `True` |
| `features` | `string_list` | no | `[]` |
| `git` | `string` | no | `""` |
| `lib` | `boolean` | no | `False` |
| `package` | `string` | yes |  |
| `package_alias` | `string` | no | `""` |
| `path` | `string` | no | `""` |
| `repositories` | `string_list` | no | `[]` |
| `rev` | `string` | no | `""` |
| `tag` | `string` | no | `""` |
| `version` | `string` | no | `""` |

Tag `cargo.splicing_config`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `repositories` | `string_list` | no | `[]` |
| `resolver_version` | `string` | no | `"2"` |

#### `rust`

Module extension.

Tag `rust.repository_set`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `allocator_library` | `label` | no | `None` |
| `auth` | `string_dict` | no | `{}` |
| `auth_patterns` | `string_dict` | no | `{}` |
| `dev_components` | `boolean` | no | `False` |
| `dlltool` | `label` | no | `None` |
| `edition` | `string` | no | `""` |
| `exec_compatible_with` | `label_list` | no | `[]` |
| `exec_triple` | `string` | no | `""` |
| `extra_exec_rustc_flags` | `string_list` | no | `[]` |
| `extra_rustc_flags` | `string_list` | no | `[]` |
| `global_allocator_library` | `label` | no | `None` |
| `name` | `name` | no | `""` |
| `netrc` | `string` | no | `""` |
| `opt_level` | `string_dict` | no | `{}` |
| `rustfmt_version` | `string` | no | `"nightly/2026-10-15"` |
| `sha256s` | `string_dict` | no | `{}` |
| `strip_level` | `string_dict` | no | `{}` |
| `target_compatible_with` | `label_list` | no | `[]` |
| `target_settings` | `label_list` | no | `[]` |
| `target_triple` | `string` | no | `""` |
| `urls` | `string_list` | no | `["https://static.rust-lang.org/dist/{}.tar.xz"]` |
| `versions` | `string_list` | no | `[]` |

Tag `rust.toolchain`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `aliases` | `string_dict` | no | `{}` |
| `allocator_library` | `label` | no | `None` |
| `dev_components` | `boolean` | no | `False` |
| `dlltool` | `label` | no | `None` |
| `edition` | `string` | no | `""` |
| `extra_exec_rustc_flags` | `string_list` | no | `[]` |
| `extra_exec_rustc_flags_triples` | `string_list_dict` | no | `{}` |
| `extra_rustc_flags` | `string_list` | no | `[]` |
| `extra_rustc_flags_triples` | `string_list_dict` | no | `{}` |
| `extra_target_triples` | `string_list` | no | `["aarch64-apple-darwin", "aarch64-unknown-linux-gnu", "x86_64-apple-darwin", "x86_64-unknown-linux-gnu", "wasm32-unknown-unknown", "wasm32-wasip1", "wasm32-wasip2"]` |
| `global_allocator_library` | `label` | no | `None` |
| `opt_level` | `string_dict` | no | `{}` |
| `rust_analyzer_version` | `string` | no | `""` |
| `rustfmt_toolchain_triples` | `string_dict` | no | `{}` |
| `rustfmt_version` | `string` | no | `"nightly/2026-10-15"` |
| `sha256s` | `string_dict` | no | `{}` |
| `strip_level` | `string_dict` | no | `{}` |
| `target_settings` | `label_list` | no | `[]` |
| `urls` | `string_list` | no | `["https://static.rust-lang.org/dist/{}.tar.xz"]` |
| `versions` | `string_list` | no | `["1.99.1", "nightly/2026-10-15"]` |

#### `rust_toolchains`

Module extension.

No tags.

### `@rules_dx//scala:defs.bzl`

#### `scala_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `deps` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `scala_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `scala_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `lib` | yes |  |
| `package` | no | `""` |
| `deps` | no | `[]` |
| `kwargs` | no |  |

#### `scala_fuzz_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `target_class` | no | `None` |
| `deps` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `scala_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `scala_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//scala:extensions.bzl`

#### `scala_config`

Module extension.

Tag `scala_config.settings`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `scala_version` | `string` | no | `"2.13.18"` |
| `scala_versions` | `string_list` | no | `[]` |

### `@rules_dx//sql:defs.bzl`

#### `sql_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `dialect` | `string` | no | `""` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

### `@rules_dx//svelte:defs.bzl`

#### `svelte_build`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `config` | yes |  |
| `node_modules` | yes |  |
| `package_json` | no | `None` |
| `public` | no | `None` |
| `ssr_entry` | no | `None` |
| `deps` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `svelte_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `tsconfig` | yes |  |
| `entry_points` | yes |  |
| `package` | yes |  |
| `node_modules` | yes |  |
| `package_json` | no | `None` |
| `deps` | no | `[]` |
| `kwargs` | no |  |

#### `svelte_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `svelte_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `node_modules` | yes |  |
| `data` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//swift:defs.bzl`

#### `swift_doc_shard`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `config` | `string` | no | `""` |
| `lib` | `label` | yes |  |
| `name` | `name` | yes |  |
| `package` | `string` | no | `""` |

#### `swift_interop_hint`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `exclude_hdrs` | `label_list` | no | `[]` |
| `module_map` | `label` | no | `None` |
| `module_name` | `string` | no | `""` |
| `name` | `name` | yes |  |
| `suppressed` | `boolean` | no | `False` |

#### `swift_library_group`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |

#### `swift_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `swift_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `platforms` | no | `None` |
| `kwargs` | no |  |

#### `swift_fuzz_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `copts` | no | `None` |
| `linkopts` | no | `None` |
| `kwargs` | no |  |

#### `swift_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `platforms` | no | `None` |
| `kwargs` | no |  |

#### `swift_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `visibility` | no | `None` |
| `platforms` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//swift:extensions.bzl`

#### `swift`

Module extension.

No tags.

#### `swiftpm`

Module extension.

Tag `swiftpm.lock`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `lock` | `label` | yes |  |
| `name` | `name` | yes |  |

### `@rules_dx//terraform:defs.bzl`

#### `terraform_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `providers` | `label_list` | no | `[]` |
| `srcs` | `label_list` | no | `[]` |

#### `terraform_provider`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `name` | `name` | yes |  |
| `source` | `string` | yes |  |
| `srcs` | `label_list` | yes |  |
| `target` | `string` | yes |  |
| `version` | `string` | yes |  |

### `@rules_dx//terraform:extensions.bzl`

#### `terraform`

Module extension.

Tag `terraform.lock`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `lock` | `label` | yes |  |
| `name` | `name` | yes |  |

### `@rules_dx//thrift:defs.bzl`

#### `thrift_cc_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `codegen_path` | `string` | no | `""` |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |

#### `thrift_java_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `codegen_path` | `string` | no | `""` |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |

#### `thrift_library`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `deps` | `label_list` | no | `[]` |
| `name` | `name` | yes |  |
| `srcs` | `label_list` | no | `[]` |

#### `thrift_rust_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `library` | yes |  |
| `crate_name` | no | `None` |
| `deps` | no | `None` |
| `codegen_path` | no | `None` |
| `kwargs` | no |  |

#### `ThriftInfo`

Provider.

Fields: `direct_deps`, `direct_srcs`, `direct_staged`, `label`, `stage_root`, `stage_roots`, `staged`, `transitive_includes`, `transitive_srcs`.

### `@rules_dx//typescript:defs.bzl`

#### `typescript_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `main` | no | `None` |
| `node_modules` | yes |  |
| `data` | no | `None` |
| `args` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `typescript_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `deps` | no | `None` |
| `data` | no | `None` |
| `tsconfig` | no | `None` |
| `transpiler` | no | `None` |
| `declaration` | no | `None` |
| `size` | no | `"small"` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `typescript_config`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `kwargs` | no |  |

#### `typescript_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `tsconfig` | yes |  |
| `entry_points` | yes |  |
| `package` | yes |  |
| `deps` | no | `[]` |
| `options` | no | `[]` |
| `kwargs` | no |  |

#### `typescript_fuzz_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `main` | no | `None` |
| `node_modules` | yes |  |
| `data` | no | `None` |
| `deps` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `typescript_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `declaration_srcs` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `typescript_playwright_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `config` | yes |  |
| `node_modules` | yes |  |
| `app` | no | `None` |
| `data` | no | `None` |
| `package_json` | no | `None` |
| `visibility` | no | `None` |
| `env_inherit` | no | `None` |
| `kwargs` | no |  |

#### `typescript_proto_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `proto` | yes |  |
| `deps` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `typescript_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `runner` | no | `"jest"` |
| `node_modules` | no | `None` |
| `data` | no | `None` |
| `deps` | no | `None` |
| `tsconfig` | no | `None` |
| `transpiler` | no | `None` |
| `declaration` | no | `None` |
| `declaration_srcs` | no | `None` |
| `package_json` | no | `None` |
| `visibility` | no | `None` |
| `env_inherit` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//typescript:extensions.bzl`

#### `typescript`

Module extension.

No tags.

#### `typescript_playwright`

Module extension.

No tags.

### `@rules_dx//vue:defs.bzl`

#### `vue_build`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `config` | yes |  |
| `node_modules` | yes |  |
| `package_json` | no | `None` |
| `public` | no | `None` |
| `ssr_entry` | no | `None` |
| `deps` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `vue_doc_shard`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `tsconfig` | yes |  |
| `entry_points` | yes |  |
| `package` | yes |  |
| `node_modules` | yes |  |
| `package_json` | no | `None` |
| `deps` | no | `[]` |
| `kwargs` | no |  |

#### `vue_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `vue_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | yes |  |
| `node_modules` | yes |  |
| `data` | no | `None` |
| `kwargs` | no |  |

### `@rules_dx//zig:defs.bzl`

#### `zig_doc_shard`

Rule.

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `config` | `string` | no | `""` |
| `lib` | `label` | yes |  |
| `name` | `name` | yes |  |
| `package` | `string` | no | `""` |

#### `zig_bench`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `visibility` | no | `None` |
| `tags` | no | `None` |
| `kwargs` | no |  |

#### `zig_binary`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `visibility` | no | `None` |
| `allow_release_fast` | no | `False` |
| `kwargs` | no |  |

#### `zig_library`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `main` | yes |  |
| `srcs` | no | `None` |
| `import_name` | no | `None` |
| `visibility` | no | `None` |
| `kwargs` | no |  |

#### `zig_test`

Macro or function.

| Argument | Required | Default |
| --- | --- | --- |
| `name` | yes |  |
| `srcs` | no | `None` |
| `main` | no | `None` |
| `visibility` | no | `None` |
| `allow_release_fast` | no | `False` |
| `kwargs` | no |  |

### `@rules_dx//zig:extensions.bzl`

#### `zig`

Module extension.

Tag `zig.lock`:

| Attribute | Type | Required | Default |
| --- | --- | --- | --- |
| `lock` | `label` | yes |  |
| `name` | `name` | yes |  |

## Build settings

| Setting | Kind | Default | Values |
| --- | --- | --- | --- |
| `@rules_dx//config:buf_breaking_against` | `label_flag` | `@rules_dx//config:buf_breaking_none` |  |
| `@rules_dx//config:clippy_flag` | `alias` | `@rules_rust//rust/settings:clippy_flag` |  |
| `@rules_dx//config:clippy_output_diagnostics` | `alias` | `@rules_rust//rust/settings:clippy_output_diagnostics` |  |
| `@rules_dx//config:config_tier` | `string_flag` | `strict` | `standard`, `strict` |
| `@rules_dx//config:extra_rustc_flag` | `alias` | `@rules_rust//rust/settings:extra_rustc_flag` |  |
| `@rules_dx//config:rustc_output_diagnostics` | `alias` | `@rules_rust//rust/settings:rustc_output_diagnostics` |  |
| `@rules_dx//config:tool_actionlint` | `label_flag` | `@dx_tools//:actionlint` |  |
| `@rules_dx//config:tool_avro` | `label_flag` | pinned |  |
| `@rules_dx//config:tool_capnp` | `label_flag` | `@dx_prebuilt//:capnp` |  |
| `@rules_dx//config:tool_container_structure_test` | `label_flag` | `@dx_tools//:container_structure_test` |  |
| `@rules_dx//config:tool_credo` | `label_flag` | pinned |  |
| `@rules_dx//config:tool_dart` | `label_flag` | `@rules_dx//config:no_dart` |  |
| `@rules_dx//config:tool_dialyzer` | `label_flag` | pinned |  |
| `@rules_dx//config:tool_elvis` | `label_flag` | pinned |  |
| `@rules_dx//config:tool_erlfmt` | `label_flag` | pinned |  |
| `@rules_dx//config:tool_errcheck` | `label_flag` | `@dx_prebuilt//:dx_errcheck` |  |
| `@rules_dx//config:tool_flatc` | `label_flag` | `@dx_prebuilt//:flatc` |  |
| `@rules_dx//config:tool_gersemi` | `label_flag` | `@dx_tools//:gersemi` |  |
| `@rules_dx//config:tool_gofumpt` | `label_flag` | `@dx_prebuilt//:dx_gofumpt` |  |
| `@rules_dx//config:tool_gotype` | `label_flag` | `@dx_prebuilt//:dx_gotype` |  |
| `@rules_dx//config:tool_govet` | `label_flag` | `@dx_prebuilt//:dx_govet` |  |
| `@rules_dx//config:tool_hadolint` | `label_flag` | `@dx_tools//:hadolint` |  |
| `@rules_dx//config:tool_helm` | `label_flag` | `@dx_tools//:helm` |  |
| `@rules_dx//config:tool_helmfmt` | `label_flag` | `@dx_tools//:helmfmt` |  |
| `@rules_dx//config:tool_hlint` | `label_flag` | pinned |  |
| `@rules_dx//config:tool_jet` | `label_flag` | pinned |  |
| `@rules_dx//config:tool_julia_format` | `label_flag` | `@rules_dx//config:no_julia_format` |  |
| `@rules_dx//config:tool_julialint` | `label_flag` | pinned |  |
| `@rules_dx//config:tool_kubeconform` | `label_flag` | `@dx_tools//:kubeconform` |  |
| `@rules_dx//config:tool_luals` | `label_flag` | pinned |  |
| `@rules_dx//config:tool_mix_format` | `label_flag` | pinned |  |
| `@rules_dx//config:tool_modfmt` | `label_flag` | `@dx_prebuilt//:dx_modfmt` |  |
| `@rules_dx//config:tool_nixfmt` | `label_flag` | `@rules_dx//core/quality/tools/nixfmt/bin:nixfmt` |  |
| `@rules_dx//config:tool_opa` | `label_flag` | `@dx_tools//:opa` |  |
| `@rules_dx//config:tool_ormolu` | `label_flag` | pinned |  |
| `@rules_dx//config:tool_perlcritic` | `label_flag` | `@rules_dx//config:no_perlcritic` |  |
| `@rules_dx//config:tool_perltidy` | `label_flag` | `@rules_dx//config:no_perltidy` |  |
| `@rules_dx//config:tool_phpcs` | `label_flag` | pinned |  |
| `@rules_dx//config:tool_phpstan` | `label_flag` | pinned |  |
| `@rules_dx//config:tool_pkl` | `label_flag` | `@dx_tools//:pkl` |  |
| `@rules_dx//config:tool_pydoclint` | `label_flag` | `@rules_dx//core/quality/tools/python:pydoclint` |  |
| `@rules_dx//config:tool_qmlformat` | `label_flag` | `@rules_dx//config:no_qmlformat` |  |
| `@rules_dx//config:tool_qmllint` | `label_flag` | `@rules_dx//config:no_qmllint` |  |
| `@rules_dx//config:tool_regal` | `label_flag` | `@dx_tools//:regal` |  |
| `@rules_dx//config:tool_ruff` | `label_flag` | `@dx_tools//:ruff` |  |
| `@rules_dx//config:tool_sorbet` | `label_flag` | pinned |  |
| `@rules_dx//config:tool_staticcheck` | `label_flag` | `@dx_prebuilt//:dx_staticcheck` |  |
| `@rules_dx//config:tool_stylua` | `label_flag` | `@dx_tools//:stylua` |  |
| `@rules_dx//config:tool_swift_format` | `label_flag` | `@rules_dx//config:no_swift_format` |  |
| `@rules_dx//config:tool_swift_lint` | `label_flag` | `@rules_dx//config:no_swift_lint` |  |
| `@rules_dx//config:tool_terraform` | `label_flag` | `@dx_tools//:terraform` |  |
| `@rules_dx//config:tool_thrift` | `label_flag` | `@dx_prebuilt//:thrift` |  |
| `@rules_dx//config:tool_trivy` | `label_flag` | `@dx_tools//:trivy` |  |
| `@rules_dx//config:tool_ty` | `label_flag` | `@dx_tools//:ty` |  |
| `@rules_dx//config:tool_weeder` | `label_flag` | pinned |  |
| `@rules_dx//config:tool_zig_ast_check` | `label_flag` | `@rules_dx//config:no_zig_ast_check` |  |
| `@rules_dx//config:tool_zig_fmt` | `label_flag` | `@rules_dx//config:no_zig_fmt` |  |
| `@rules_dx//config:toolchain_linker_preference` | `alias` | `@rules_rust//rust/settings:toolchain_linker_preference` |  |
| `@rules_dx//config:toolchains` | `bool_flag` | `False` |  |
| `@rules_dx//config:warnings_as_errors` | `bool_flag` | `True` |  |
| `@rules_dx//config:workspace` | `label_flag` | `@rules_dx//config:default_workspace_policy` |  |
| `@rules_dx//android:emulator_serial` | `string_flag` |  |  |
| `@rules_dx//apple:simulator_runtime` | `string_flag` |  |  |
| `@rules_dx//cc:compiler` | `string_flag` | `llvm` |  |
| `@rules_dx//cuda:gpu_compute_capability` | `string_flag` |  |  |
| `@rules_dx//cuda:gpu_driver` | `string_flag` |  |  |
| `@rules_dx//fortran:kcov` | `bool_flag` | `False` |  |
| `@rules_dx//zig:kcov` | `bool_flag` | `False` |  |

## Platforms

Pass a platform to Bazel with `--platforms=<platform>`.

- `@rules_dx//android:android_device`
- `@rules_dx//android:android_emulator`
- `@rules_dx//android:arm64-v8a`
- `@rules_dx//android:x86_64`
- `@rules_dx//apple:macos_arm64`
- `@rules_dx//apple:macos_x86_64`
- `@rules_dx//deploy:linux_amd64`
- `@rules_dx//deploy:linux_arm64`
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

- `@rules_dx//cuda:toolchain_type`
- `@rules_dx//d:toolchain_type`
- `@rules_dx//elixir:toolchain_type`
- `@rules_dx//erlang:exec_toolchain_type`
- `@rules_dx//erlang:toolchain_type`
- `@rules_dx//fortran:toolchain_type`
- `@rules_dx//haskell:toolchain_type`
- `@rules_dx//lua:toolchain_type`
- `@rules_dx//msix:toolchain_type`
- `@rules_dx//ocaml:exec_toolchain_type`
- `@rules_dx//ocaml:profile_toolchain_type`
- `@rules_dx//ocaml:toolchain_type`
- `@rules_dx//php:exec_toolchain_type`
- `@rules_dx//php:toolchain_type`

## Hub repositories

| Repository | Extension |
| --- | --- |
| `@*` | `@rules_dx//go:extensions.bzl%go` |
| `@ansible_collections` | `@rules_dx//ansible:extensions.bzl%ansible` |
| `@bicep` | `@rules_dx//bicep:extensions.bzl%bicep` |
| `@composer` | `@rules_dx//php:extensions.bzl%composer` |
| `@cpan` | `@rules_dx//perl:extensions.bzl%cpan` |
| `@crates` | `@rules_dx//rust:extensions.bzl%cargo` |
| `@dub` | `@rules_dx//d:extensions.bzl%dub` |
| `@dx_terraform_providers` | `@rules_dx//terraform:extensions.bzl%terraform` |
| `@fpm` | `@rules_dx//fortran:extensions.bzl%fpm` |
| `@gems` | `@rules_dx//ruby:extensions.bzl%ruby` |
| `@hackage` | `@rules_dx//haskell:extensions.bzl%cabal` |
| `@hex` | `@rules_dx//elixir:extensions.bzl%hex` |
| `@julia` | `@rules_dx//julia:extensions.bzl%julia` |
| `@maven` | `@rules_dx//java:extensions.bzl%maven` |
| `@npm` | `@rules_dx//javascript:extensions.bzl%npm` |
| `@nuget` | `@rules_dx//csharp:extensions.bzl%nuget` |
| `@opam` | `@rules_dx//ocaml:extensions.bzl%opam` |
| `@pnpm` | `@rules_dx//javascript:extensions.bzl%pnpm` |
| `@psgallery` | `@rules_dx//powershell:extensions.bzl%powershell` |
| `@pub_deps` | `@rules_dx//dart:extensions.bzl%pub` |
| `@pypi` | `@rules_dx//python:extensions.bzl%uv` |
| `@rocks` | `@rules_dx//lua:extensions.bzl%luarocks` |
| `@swiftpkg` | `@rules_dx//swift:extensions.bzl%swiftpm` |
| `@zig` | `@rules_dx//zig:extensions.bzl%zig` |

## Targets

| Target | Kind |
| --- | --- |
| `@rules_dx//:package_json` | `javascript_library` |
| `@rules_dx//android:declared_emulator` | `declared_emulator` |
| `@rules_dx//android:emulator_undeclared` | `config_setting` |
| `@rules_dx//android:is_device` | `config_setting` |
| `@rules_dx//android:is_emulator` | `config_setting` |
| `@rules_dx//android:llvm_readelf` | `alias` |
| `@rules_dx//apple:declared_simulator` | `declared_simulator` |
| `@rules_dx//apple:ios_arm64` | `alias` |
| `@rules_dx//apple:ios_sim_arm64` | `alias` |
| `@rules_dx//apple:ios_x86_64` | `alias` |
| `@rules_dx//apple:simulator_undeclared` | `config_setting` |
| `@rules_dx//capnproto:runtime_java` | `alias` |
| `@rules_dx//capnproto:runtime_rust` | `alias` |
| `@rules_dx//capnproto:std` | `filegroup` |
| `@rules_dx//cc:gcc_linux_glibc` | `alias` |
| `@rules_dx//cc:gcc_linux_libstdcxx` | `alias` |
| `@rules_dx//cuda:cublas` | `cuda_component_library` |
| `@rules_dx//cuda:cufft` | `cuda_component_library` |
| `@rules_dx//cuda:curand` | `cuda_component_library` |
| `@rules_dx//cuda:gpu_undeclared` | `config_setting` |
| `@rules_dx//d:coverage` | `config_setting` |
| `@rules_dx//dart:runfiles` | `alias` |
| `@rules_dx//dx/launch:dx` | `alias` |
| `@rules_dx//flatbuffers:reflection_fbs_schema` | `alias` |
| `@rules_dx//flatbuffers:runtime_java` | `alias` |
| `@rules_dx//flatbuffers:runtime_rust` | `alias` |
| `@rules_dx//flutter:profile` | `alias` |
| `@rules_dx//fortran:kcov_build` | `config_setting` |
| `@rules_dx//haskell:base` | `alias` |
| `@rules_dx//ocaml:bisect_ppx_rewriter` | `alias` |
| `@rules_dx//ocaml:dune` | `alias` |
| `@rules_dx//ocaml:emitter` | `alias` |
| `@rules_dx//php:runfiles` | `php_library` |
| `@rules_dx//powershell:dx_coverage` | `alias` |
| `@rules_dx//powershell:pester` | `alias` |
| `@rules_dx//powershell:runfiles` | `powershell_library` |
| `@rules_dx//rust:build_script_cc_env` | `build_script_cc_env` |
| `@rules_dx//rust:prost` | `alias` |
| `@rules_dx//rust:prost_types` | `alias` |
| `@rules_dx//rust:tokio` | `alias` |
| `@rules_dx//rust:tokio_stream` | `alias` |
| `@rules_dx//rust:tonic` | `alias` |
| `@rules_dx//rust:wasi_run` | `rust_library` |
| `@rules_dx//rust:wasmtime` | `alias` |
| `@rules_dx//thrift:thrift_compiler` | `alias` |
| `@rules_dx//zig:kcov_build` | `config_setting` |
| `@rules_dx//zig:sancov_build` | `config_setting` |

## Runnable targets

Run these with `bazel run <target>`.

| Target | Kind |
| --- | --- |
| `@rules_dx//cli/cli:dx` | `rust_binary` |
| `@rules_dx//cli/codegen_shard:codegen_shard_writer` | `rust_binary` |
| `@rules_dx//cli/compile_check:compile_check` | `rust_binary` |
| `@rules_dx//cli/coverage_summary:coverage_summary` | `rust_binary` |
| `@rules_dx//cli/env:env` | `rust_binary` |
| `@rules_dx//cli/launcher:exec_launcher` | `rust_binary` |
| `@rules_dx//cli/ocaml_cc_driver:ocaml_dune` | `rust_binary` |
| `@rules_dx//cli/prebuilt:guard_stub` | `cc_binary` |
| `@rules_dx//cli/release_tree:release_tree_subcommand` | `rust_binary` |
| `@rules_dx//cli/runtime:dx_runtime` | `rust_binary` |
| `@rules_dx//core/quality/capture:capture` | `rust_binary` |
| `@rules_dx//core/quality/lsp_check:lsp_check` | `rust_binary` |
| `@rules_dx//core/quality/markdown:quality_markdown` | `rust_binary` |
| `@rules_dx//core/quality/notebook:quality_notebook` | `rust_binary` |
| `@rules_dx//core/quality/runner:quality_runner` | `rust_binary` |
| `@rules_dx//core/quality/tools/diff:buf_diff` | `rust_binary` |
| `@rules_dx//core/quality/tools/python:ansible_lint` | `py_binary` |
| `@rules_dx//core/quality/tools/python:cfn_lint` | `py_binary` |
| `@rules_dx//core/quality/tools/python:check_jsonschema` | `py_binary` |
| `@rules_dx//core/quality/tools/python:cmakelint` | `py_binary` |
| `@rules_dx//core/quality/tools/python:djlint` | `py_binary` |
| `@rules_dx//core/quality/tools/python:docstrfmt` | `py_binary` |
| `@rules_dx//core/quality/tools/python:mbake` | `py_binary` |
| `@rules_dx//core/quality/tools/python:pydoclint` | `py_binary` |
| `@rules_dx//core/quality/tools/python:pypi_attestations` | `py_binary` |
| `@rules_dx//core/quality/tools/python:rst2html` | `py_binary` |
| `@rules_dx//core/quality/tools/python:sqlfluff` | `py_binary` |
| `@rules_dx//core/quality/tools/python:yamllint` | `py_binary` |
| `@rules_dx//deploy:bootstrap_offline` | `rust_binary` |
| `@rules_dx//examples/shared-rust:shared_tasks` | `rust_binary` |
| `@rules_dx//examples/shared-rust:tasks` | `rust_binary` |
| `@rules_dx//examples/shared-rust:tasks_wasi` | `rust_binary` |

## File formats

| Schema | Version |
| --- | --- |
| `events.schema.json` | `4.2` |
| `dx.schema.json` | `1.2` |
