# Environment, Codegen, And Setup Commands

```sh
bazel run //dx:env
bazel run @rules_dx//:dx -- setup
bazel run @rules_dx//:dx -- env
bazel run @rules_dx//:dx -- codegen
```

```text
dx env [--check] [--apply] [<label>] [-- bazel-options...]
dx codegen [--check] [--apply] [<label>] [-- bazel-options...]
dx setup [--check] [--apply] [<label>] [-- bazel-options...]
```

`dx setup` prepares codegen plus env in one go. `dx env` refreshes the
managed tools. `dx codegen` collects generated sources. Repository-wide by
default. With one exact `//` or `@` label, each acts on that target only.

All three check the intended selection by default and report drift without
writing. Pass `--apply` to select it. A missing selection is drift, not
success. Checking never selects a new generation, populates managed
bins, or commits records.

These commands keep managed state under `.dx/`, which is gitignored and
pruned by `dx clean`. They take no `--fail-on` or `--report`.

Output: `--output text|json`.

Exit codes: `0` success, `2` usage or scope errors, `1` drift, a launch or
commit failure. Bazel failures keep Bazel's code.

```sh
bazel run @rules_dx//:dx -- env //a:one
bazel run @rules_dx//:dx -- setup --dry-run
bazel run @rules_dx//:dx -- env --apply
```
