# `dx deploy`

Builds one deployable target and runs its dry run. `--apply` publishes.

```sh
dx deploy //app:deploy
dx deploy --release //app:deploy
NPM_PUBLISH_APPROVED=1 dx deploy --apply //pkg:npm
```

```text
dx deploy [--apply] [--debug|--release] <label> [-- args...]
```

| Flag | What it does |
| --- | --- |
| `--apply` | Publish. Sets `DX_DEPLOY_LIVE=1` for the launcher. |
| `--debug` | Compile with the `dx_build_debug` build mode. |
| `--release` | Compile with the `dx_build_release` build mode. |
| `--workspace`, `-q`, `-v`, `--color`, `--build-slots`, `--build-slots-wait`, `--bazel-startup-option` | Global flags. |

`--debug` and `--release` conflict. Without either, the build mode comes from the target's `build_mode` attribute. Both runs set `DX_BUILD_MODE`.

Scopes: one label. When the label is missing or is a pattern, the error lists the repo's `dx_deployment` labels and marks the ones tagged `release`. Args after `--` reach the launcher.

The deploy rules, their attributes and their credentials are in the [deploy rules reference](https://github.com/devhusdk/rules_dx/blob/main/deploy/README.md).

## Dry run

Without `--apply`, dx runs the launcher with `DX_DEPLOY_LIVE=0`. The launcher stages, validates and prints what it would publish, and contacts no registry. A plain executable without a deploy launcher is only built.

## Publish

`--apply` runs the launcher with `DX_DEPLOY_LIVE=1`. The launcher also needs its `<X>_APPROVED=1` variable and its credentials from the shell or CI:

```sh
NPM_PUBLISH_APPROVED=1 tools/dx deploy --apply //pkg:npm
```

## Agent sessions

In an agent session (`CLAUDECODE`, `CODEX_THREAD_ID` and the other agent variables), `--apply` without `CI=true` exits `1` with `deploy_needs_human`: a human or CI publishes. Run the dry run instead.

- `dx init --agents=claude` adds `permissions.ask` rules for `tools/dx deploy --apply` to `.claude/settings.json`.
- The agent guard hook asks first (Claude, Copilot) or refuses (Codex, Gemini, Cursor) when a command runs `dx deploy` with `--apply`, in any word order.

## Registries released by tag

Go, Julia, Swift and dub have no deploy rule. Release them with a git tag:

- Go: push the tag `vX.Y.Z`.
- Swift, dub: push a semver tag.
- Julia: push a tag, then comment `@JuliaRegistrator register` on the commit.

PHP, opam and CPAN have deploy rules: `packagist_deploy`, `opam_deploy` and `cpan_deploy`.

## Output

Text only. The launcher's output is printed as it runs.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | Operational failure or `deploy_needs_human`. |
| `2` | Usage error: no label, a pattern, or several labels. |
| Bazel's code | The build failed. |
| The launcher's code | The launcher exited nonzero. |
| `128`+N | The launcher or dx died by signal N. |
