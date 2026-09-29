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
dx lint [--here] [--check] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...]
dx typecheck [--here] [--check] [--fail-on info|warning|error] [--report sarif=<dest>] [scope...]
dx format [--here] [--check] [--fail-on info|warning|error] [scope...]
```

- `--check`: report findings without writing files.
- `--fail-on info|warning|error`: severity that fails. Default `warning`.
- `--report sarif=<dest>`: write a SARIF report for `lint` and `typecheck`.
  Repeatable. `dx format` has no report format.
- `--output text|diff|json`: result shape.

Exit codes: `0` success, `2` usage or scope errors, `1` findings at or above
`--fail-on`, an incomplete result set, or a failed report write. Bazel
failures report `1`, not Bazel's code.

```sh
bazel run //cli/cli:dx -- lint --check //...
bazel run //cli/cli:dx -- format --here
bazel run //cli/cli:dx -- typecheck --fail-on error //cli/...
```
