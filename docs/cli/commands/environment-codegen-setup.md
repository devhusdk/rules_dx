# Environment, Codegen, And Setup Commands

```sh
bazel run //dx:env
bazel run //cli/cli:dx -- setup
bazel run //cli/cli:dx -- env
bazel run //cli/cli:dx -- codegen
```

```text
dx env [<label>] [-- bazel-options...]
dx codegen [<label>] [-- bazel-options...]
dx setup [<label>] [-- bazel-options...]
```

`dx setup` prepares codegen plus env in one go. `dx env` refreshes the
managed tools. `dx codegen` collects generated sources. Repository-wide by
default. With one exact `//` or `@` label, each acts on that target only.

These commands change managed state. They take no `--check`, `--fail-on`,
or `--report`. Output is text or JSON only.
