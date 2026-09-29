# Command Reference

Every `dx` command runs through Bazel. Put `dx` flags before the command,
Bazel flags after `--`.

```sh
bazel run //cli/cli:dx -- build //...
bazel run //cli/cli:dx -- lint --check //... -- --jobs=4
```

- [`dx build`, `dx test`, `dx coverage`, `dx run`, `dx deploy`](build-test-coverage.md)
- [`dx lint`, `dx typecheck`, `dx format`](quality.md)
- [`dx check`, `dx fix`, `dx clean`](check-fix-clean.md)
- [`dx generate`](generate.md)
- [`dx env`, `dx codegen`, `dx setup`](environment-codegen-setup.md)
- [`dx security`, `dx license`, `dx update`, `dx bump`, `dx bazel`](audit-update-bazel.md)
- [`dx docs`](docs.md)
- [`dx init`, `dx hooks`](hooks.md)
- [`dx status`, `dx version`](status-version.md)
- [`dx owners`, `dx deps`, `dx why`](inspect.md)
- [`dx completion`](completion.md)
- [`dx migrate`](migrate.md)
- [`dx new`, `dx upgrade`](new-upgrade.md)
- [`dx watch`](watch.md)

Scope rules live in [Scope Defaults](scope-defaults.md). There is no `dx doctor`. Use `dx status`.

## Global Flags

- `--workspace <dir>`: run in another workspace.
- `--dry-run`: print the plan without running it.
- `--quiet`, `--verbose`: less or more output.
- `--log-level error|warn|info|debug|trace`: log verbosity.
- `--color auto|always|never`: color output. Default `auto`.
- `--output text|diff|json`: result shape. `diff` is limited to `lint`,
  `typecheck`, `format`, `generate`, `check`, and `fix`.
- `--report <format>=<dest>`: write a report. Repeatable. Formats per command:
  `sarif` and `spdx` for `security` and `license`; `sarif` for `lint`,
  `typecheck`, `check`, and `fix`; `junit` for `test`; `lcov` for `coverage`.
  Every other command rejects `--report`.
- `--fail-on info|warning|error`: severity that fails. Default `warning`.
- `--check`: report without changing files, where supported.
- `--debug`, `--release`: build profiles for build-like commands.
- `--here`: limit to the current directory tree.
- `-h`, `--help`: print help for a command.
- `-V`, `--version`: print the version.

Exit codes: `0` success, `2` usage error, `1` failed check.

## Environment

- `RUST_LOG=<filter>`: overrides `--verbose` and `--log-level`.
- `NO_COLOR=<any>`: disables color output.
- `BUILD_WORKSPACE_DIRECTORY=<dir>`: workspace start under `bazel run`.
