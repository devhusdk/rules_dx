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
- `--output text|diff|json`: result shape. `diff` has no patch for most commands.
- `--report <format>=<dest>`: write SARIF, JUnit, or LCOV reports. Repeatable.
- `--fail-on info|warning|error`: severity that fails. Default `warning`.
- `--check`: report without changing files, where supported.
- `--debug`, `--release`: build profiles for build-like commands.
- `--here`: limit to the current directory tree.

Exit codes: `0` success, `2` usage error, `1` failed check.
