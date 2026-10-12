# Documentation

Start here to use `dx`. There is no `dx` install step: `dx` runs through Bazel.
Install Bazel via Bazelisk (see `.bazelversion`), then follow the command and CI
guides below.

## Commands

- [Command reference](cli/commands/README.md): behavior of each `dx` command.
- [Scope defaults](cli/commands/scope-defaults.md): labels, files, dirs, and `--here`.
- [Build, test, coverage](cli/commands/build-test-coverage.md): build, test, run, deploy, coverage.
- [Quality](cli/commands/quality.md): lint, typecheck, format.
- [Check, fix, clean](cli/commands/check-fix-clean.md): fix-up loops and cleanup.
- [Generate](cli/commands/generate.md): refresh `BUILD` files.
- [Docs](cli/commands/docs.md): build, check, and preview this site.
- [Environment, codegen, setup](cli/commands/environment-codegen-setup.md): `dx env`, `dx codegen`, `dx setup`.
- [Audit, update, Bazel](cli/commands/audit-update-bazel.md): `dx security`, `dx license`, `dx update`, `dx bump`, `dx bazel`.
- [Init, hooks](cli/commands/hooks.md): `dx init`, `dx hooks`.
- [Status, version](cli/commands/status-version.md): `dx status`, `dx version`.
- [Inspect](cli/commands/inspect.md): `dx owners`, `dx deps`, `dx why`.
- [Tests](cli/commands/tests.md): `dx tests`.
- [Completion](cli/commands/completion.md): `dx completion`.
- [Capabilities](cli/commands/capabilities.md): `dx capabilities`.
- [Verify](cli/commands/verify.md): `dx verify`.
- [Rerun](cli/commands/rerun.md): `dx rerun`.
- [Migrate](cli/commands/migrate.md): `dx migrate`.
- [New, upgrade](cli/commands/new-upgrade.md): `dx new`, `dx upgrade`.
- [Watch](cli/commands/watch.md): `dx watch`.

## CI And Examples

- [GitHub CI](github-ci.md): run `dx` in GitHub Actions.
- [Examples](../examples/README.md): starter callers and per-language adoption workspaces.
- [Docs site](https://ralvik.github.io/rules_dx/): rendered reference.
