# Quality Commands

```sh
bazel run //cli/cli:dx -- lint //...
bazel run //cli/cli:dx -- typecheck //...
bazel run //cli/cli:dx -- format //...
```

No scope means `//...`. Use `--here` for the current dir tree. All three
rewrite files by default and report with `--check`. Args after `--` go to
Bazel unchanged.

```text
dx lint [--here] [--check] [--fail-on info|warning|error] [--report <format>=<dest>] [scope...]
dx typecheck [--here] [--check] [--fail-on info|warning|error] [--report <format>=<dest>] [scope...]
dx format [--here] [--check] [--fail-on info|warning|error] [--report <format>=<dest>] [scope...]
```

- `--check`: report findings without writing files.
- `--fail-on info|warning|error`: severity that fails. Default `warning`.
- `--report <format>=<dest>`: write SARIF or JUnit reports. Repeatable.
- `--output text|diff|json`: result shape.

```sh
bazel run //cli/cli:dx -- lint --check //...
bazel run //cli/cli:dx -- format --here
bazel run //cli/cli:dx -- typecheck --fail-on error //cli/...
```
