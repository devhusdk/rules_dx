# `dx deploy`

Use `dx deploy` to check what a deploy target would publish, then publish it with `--apply`.

A target is one thing Bazel builds. A label names it, such as `//app:deploy`: the package directory `app`, then the target name. A deploy target is a `dx_deployment` or one of the deploy rules in the [deploy rules reference](https://github.com/devhusdk/rules_dx/blob/main/deploy/README.md).

## Examples

Dry run one target. Nothing is published:

```sh
dx deploy //app:deploy
```

Publish it:

```sh
NPM_PUBLISH_APPROVED=1 dx deploy --apply //pkg:npm
```

Dry run with a debug build:

```sh
dx deploy --debug //app:deploy
```

Pass arguments to the deploy launcher after `--`:

```sh
dx deploy //app:deploy -- <launcher-args>
```

Deploy every release deployment your change reaches:

```sh
dx deploy --affected
dx deploy --apply --affected --since origin/release
```

## Usage

```text
dx deploy [--apply] [--debug|--release] <label> [-- app-args ...]
dx deploy [--apply] [--debug|--release] --affected [--since <ref>]
```

## Dry run

Without `--apply`, dx builds the target, then runs its launcher with `DX_DEPLOY_LIVE=0`. The launcher stages, validates and prints what it would publish. It contacts no registry.

A plain executable target that is not a deploy rule is only built in a dry run.

## Publish

With `--apply`, dx runs the launcher with `DX_DEPLOY_LIVE=1`. The launcher also needs its approval variable, such as `NPM_PUBLISH_APPROVED=1`, and its registry credentials from your shell or CI. The deploy rules reference lists both for each rule.

```sh
NPM_PUBLISH_APPROVED=1 tools/dx deploy --apply //pkg:npm
```

`--apply` on a plain executable target runs it with `DX_DEPLOY_LIVE=1`.

## Affected deployments

`--affected` finds the files changed since the merge base with the default branch. It then deploys every `dx_deployment` tagged `release` that those changes reach, one after another. `--since <ref>` compares against `<ref>` instead. `--affected` takes no label. Targets under `[scope] exclude` in `dx.toml` are skipped. When nothing is affected, dx prints `no affected targets` and exits `0`.

## Agent sessions

In an AI agent session, such as Claude Code or Codex, `--apply` fails with `deploy_needs_human` unless `CI=true` is set. A human or CI publishes. Run the dry run instead.

- `dx init` for Claude Code adds `permissions.ask` rules for `tools/dx deploy --apply` to `.claude/settings.json`.
- The dx agent hook asks you first in Claude Code and Copilot. It refuses in Codex, Gemini and Cursor.

## Registries released by tag

Go, Julia, Swift and dub have no deploy rule. Release them with a git tag:

- Go: push the tag `vX.Y.Z`.
- Swift and dub: push a semver tag.
- Julia: push a tag, then comment `@JuliaRegistrator register` on the commit.

PHP, opam and CPAN have deploy rules: `packagist_deploy`, `opam_deploy` and `cpan_deploy`.

## Flags

| Flag | What it does |
| --- | --- |
| `--apply` | Publish. Runs the launcher with `DX_DEPLOY_LIVE=1`. |
| `--debug` | Build in debug mode. |
| `--release` | Build in release mode. |
| `--affected` | Deploy every `dx_deployment` tagged `release` that your change reaches. Takes no label. |
| `--since <ref>` | With `--affected`, compare against `<ref>` instead of the merge base with the default branch. |
| `--workspace <dir>` | Run in this workspace. |
| `-q, --quiet` | Hide summaries. |
| `-v, --verbose` | Show more logs. |
| `--color auto\|always\|never` | Set color output. |
| `--build-slots auto\|<n>\|off` | Set build slots. |
| `--build-slots-wait <duration>` | Give up waiting for a build slot after this long. `0` fails at once. |
| `--bazel-startup-option <token>` | Pass one Bazel startup option. Repeatable. |

`--debug` and `--release` conflict. The build mode is the flag you pass, else the target's `build_mode` attribute, else release. The launcher sees it in `DX_BUILD_MODE`.

## Scopes

Exactly one label in your workspace, such as `//app:deploy`. A pattern like `//...`, several labels, or a file path is a usage error. The error lists every `dx_deployment` label in the repository and marks the ones tagged `release`.

## Output

Text only. dx prints a summary line, then the build and the launcher output as they run. A dry run also prints:

- `Size of <label>: ...` when the target sets a size budget.
- `SBOM of <label>: <file> <n> packages` when the target has an SBOM.
- `Dry run of <label> (nothing published; publish with: <command>)`.
- `Validated <label> (built, not published; publish with: <command>)` for a plain executable target.

`--quiet` hides these lines. Reports: none.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | Operational failure, or `deploy_needs_human`. |
| `2` | Usage or scope error: no label, a pattern, several labels, or a target that is neither a deploy rule nor executable. |
| Bazel's code | The build failed. |
| The launcher's code | The launcher exited nonzero. |
| `128`+N | Signal N stopped the launcher or dx. |

With `--affected`, every deployment runs, and dx exits with the code of the first one that failed.
