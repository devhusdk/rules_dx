# Environment And Codegen Commands

```sh
bazel run @rules_dx//:dx -- env
bazel run @rules_dx//:dx -- codegen
```

```text
dx env [--check] [--apply] [<label>] [-- bazel-options...]
dx codegen [--check] [--apply] [<label>] [-- bazel-options...]
```

`dx env` refreshes the managed tools. `dx codegen` collects generated
sources. Repository-wide by default. With one exact `//` or `@` label, each
acts on that target only.

Both check the intended selection by default and report drift without
writing. Pass `--apply` to select it. A missing selection is drift, not
success. Checking never selects a new generation, populates managed
bins, or commits records.

These commands keep managed state under `.dx/`, which is gitignored and
pruned by `dx clean`. They take no `--fail-on` or `--report`.

To run both, run `env` first, then `codegen`. Stop on the first failure.

```sh
bazel run @rules_dx//:dx -- env --apply
bazel run @rules_dx//:dx -- codegen --apply
```

`dx setup` is not a command. It exits `2` and names `dx env` and
`dx codegen`.

Output: `--output text|json`.

Exit codes: `0` success, `2` usage or scope errors, `1` drift, a launch or
commit failure. Bazel failures keep Bazel's code.

```sh
bazel run @rules_dx//:dx -- env //a:one
bazel run @rules_dx//:dx -- codegen --apply
bazel run @rules_dx//:dx -- env --apply
```
