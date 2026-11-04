# Documentation

Start here to use `dx`. `dx` runs through Bazel. Install Bazel via
Bazelisk (see `.bazelversion`), then follow the command and CI guides below.

- [Tutorial](tutorial.md): from an empty directory to a passing `dx test`.
- [Bazel for dx users](bazel-for-dx-users.md): sandbox, runfiles, caching, logs, visibility.
- [Adopting rules_dx in an existing Bazel repo](adopting-existing-bazel.md): move from upstream rules one package at a time.
- [Adopting rules_dx without Bazel experience](adopting-without-bazel.md): use `dx` in a Cargo, Python or JavaScript repo.

## Guides

- [Use a remote cache](remote-cache.md): share build outputs with your team and CI.
- [Use JetBrains IDEs and other editors](editors.md): point editors at the files `dx` writes.

## Support

- [Licenses](licenses.md): the licence of every tool rules_dx downloads.
- [Support matrix](support-matrix.md): the platforms CI tests each language on and its quality tools. An empty cell or `no` is a missing test, tracked as an issue.

## Languages

Per-language guides in the repository:

- [Android](https://github.com/devhusdk/rules_dx/blob/main/android/README.md)
- [AppImage](https://github.com/devhusdk/rules_dx/blob/main/appimage/README.md)
- [Apple](https://github.com/devhusdk/rules_dx/blob/main/apple/README.md)
- [C#](https://github.com/devhusdk/rules_dx/blob/main/csharp/README.md)
- [C/C++](https://github.com/devhusdk/rules_dx/blob/main/cc/rules/README.md)
- [CUDA](https://github.com/devhusdk/rules_dx/blob/main/cuda/README.md)
- [D](https://github.com/devhusdk/rules_dx/blob/main/d/README.md)
- [Dart](https://github.com/devhusdk/rules_dx/blob/main/dart/README.md)
- [Elixir](https://github.com/devhusdk/rules_dx/blob/main/elixir/README.md)
- [Erlang](https://github.com/devhusdk/rules_dx/blob/main/erlang/README.md)
- [Flutter](https://github.com/devhusdk/rules_dx/blob/main/flutter/README.md)
- [Fortran](https://github.com/devhusdk/rules_dx/blob/main/fortran/README.md)
- [F#](https://github.com/devhusdk/rules_dx/blob/main/fsharp/README.md)
- [Go](https://github.com/devhusdk/rules_dx/blob/main/go/README.md)
- [Haskell](https://github.com/devhusdk/rules_dx/blob/main/haskell/README.md)
- [Julia](https://github.com/devhusdk/rules_dx/blob/main/julia/README.md)
- [Lua](https://github.com/devhusdk/rules_dx/blob/main/lua/README.md)
- [MSIX](https://github.com/devhusdk/rules_dx/blob/main/msix/README.md)
- [OCaml](https://github.com/devhusdk/rules_dx/blob/main/ocaml/README.md)
- [Perl](https://github.com/devhusdk/rules_dx/blob/main/perl/README.md)
- [PHP](https://github.com/devhusdk/rules_dx/blob/main/php/README.md)
- [PowerShell](https://github.com/devhusdk/rules_dx/blob/main/powershell/README.md)
- [Python](https://github.com/devhusdk/rules_dx/blob/main/python/README.md)
- [Qt](https://github.com/devhusdk/rules_dx/blob/main/qt/README.md)
- [Ruby](https://github.com/devhusdk/rules_dx/blob/main/ruby/README.md)
- [Swift](https://github.com/devhusdk/rules_dx/blob/main/swift/README.md)
- [Zig](https://github.com/devhusdk/rules_dx/blob/main/zig/README.md)

## Commands

- [Command reference](cli/commands/README.md): behavior of each `dx` command.
- [Supported surface](cli/commands/README.md#supported-surface): labels and `.bzl` files.
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
