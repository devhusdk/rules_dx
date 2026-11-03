# Documentation

Start here to use `dx`. `dx` runs through Bazel. To put a standalone binary on a
machine, use [`dx install`](cli/commands/install.md). Otherwise install Bazel via
Bazelisk (see `.bazelversion`), then follow the command and CI guides below.

- [Tutorial](tutorial.md): from an empty directory to a passing `dx test`.
- [Adopting rules_dx in an existing Bazel repo](adopting-existing-bazel.md): move from upstream rules one package at a time.

## Support

- [Licenses](licenses.md): the licence of every tool rules_dx downloads.
- [Support matrix](support-matrix.md): the platforms CI tests each language on and its quality tools. An empty cell or `no` is a missing test, tracked as an issue.

## Commands

- [Command reference](cli/commands/README.md): behavior of each `dx` command.
- [Supported surface](cli/commands/README.md#supported-surface): labels and `.bzl` files covered by the compatibility promise.
- [Scope defaults](cli/commands/scope-defaults.md): labels, files, dirs, and `--here`.
- [Build, test, coverage](cli/commands/build-test-coverage.md): build, test, run, deploy, coverage.
- [Quality](cli/commands/quality.md): lint, typecheck, format.
- [Check, fix, clean](cli/commands/check-fix-clean.md): fix-up loops and cleanup.
- [Generate](cli/commands/generate.md): refresh `BUILD` files.
- [Docs](cli/commands/docs.md): build, check, and preview this site.
- [Environment](cli/commands/env.md): managed tools and generated source.
- [Audit and update](cli/commands/audit-update.md): security, license, update.
- [Init](cli/commands/init.md): set up a workspace.
- [Status](cli/commands/status.md): health report and the version pin.
- [Completion](cli/commands/completion.md): shell completions.
- [Upgrade](cli/commands/upgrade.md): change the pin.
- [Offline bundle](cli/commands/offline-bundle.md): build a vendored bundle and install from it.
- [Watch](cli/commands/watch.md): rerun on file changes.
- [Worktree](cli/commands/worktree.md): worktree slots.
- [Windows](cli/commands/windows.md): what a Windows host needs before `dx` works.

## CI And Examples

- [GitHub CI](github-ci.md): run `dx` in GitHub Actions.
- [Examples](https://github.com/devhusdk/rules_dx/blob/main/examples/README.md): starter callers and per-language adoption workspaces.
- [Docs site](https://devhusdk.github.io/rules_dx/): rendered reference.
