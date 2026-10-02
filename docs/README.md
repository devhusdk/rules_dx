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

## CI And Examples

- [GitHub CI](github-ci.md): run `dx` in GitHub Actions.
- [Examples](../examples/README.md): starter callers and per-language adoption workspaces.
- [Docs site](https://ralvik.github.io/rules_dx/): rendered reference.
