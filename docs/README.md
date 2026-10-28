# Documentation

Start here to use `dx`. There is no `dx` install step: `dx` runs through Bazel.
Install Bazel via Bazelisk (see `.bazelversion`), then follow the command and CI
guides below.

- [Adopting rules_dx in an existing Bazel repo](adopting-existing-bazel.md): move from upstream rules one package at a time.

## Support

- [Support matrix](support-matrix.md): the platforms CI tests each language on, its `dx new` starter, and its quality tools. An empty cell or `no` is a missing test, tracked as an issue.

## Commands

- [Command reference](cli/commands/README.md): behavior of each `dx` command.
- [Supported surface](cli/commands/README.md#supported-surface): labels and `.bzl` files covered by the compatibility promise.
- [Scope defaults](cli/commands/scope-defaults.md): labels, files, dirs, and `--here`.
- [Build, test, coverage](cli/commands/build-test-coverage.md): build, test, run, deploy, coverage.
- [Quality](cli/commands/quality.md): lint, typecheck, format.
- [Check, fix, clean](cli/commands/check-fix-clean.md): fix-up loops and cleanup.
- [Generate](cli/commands/generate.md): refresh `BUILD` files.
- [Docs](cli/commands/docs.md): build, check, and preview this site.
- [Environment and codegen](cli/commands/environment-codegen.md): env and codegen.
- [Audit and update](cli/commands/audit-update.md): security, license, update, bump, add.
- [Init](cli/commands/init.md): set up a workspace.
- [Status and version](cli/commands/status-version.md): health report and the version pin.
- [List tests](cli/commands/list-tests.md): list test targets.
- [Completion](cli/commands/completion.md): shell completions.
- [Migrate](cli/commands/migrate.md): move to the current layout.
- [New, upgrade, rollback](cli/commands/new-upgrade.md): create modules and change the pin.
- [Offline bundle](cli/commands/offline-bundle.md): build a vendored bundle and install from it.
- [Watch](cli/commands/watch.md): rerun on file changes.
- [Worktree](cli/commands/worktree.md): worktree slots.
- [Module families](module-families.md): the plan for splitting core and family modules.

## CI And Examples

- [GitHub CI](github-ci.md): run `dx` in GitHub Actions.
- [Examples](../examples/README.md): starter callers and per-language adoption workspaces.
- [Docs site](https://devhusdk.github.io/rules_dx/): rendered reference.
- [Starlark rule reference](https://devhusdk.github.io/rules_dx/rules/): generated rule and aspect docs.
