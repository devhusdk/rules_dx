# rules_dx

`rules_dx` is a Bazel developer platform with one `dx` CLI for
build, test, lint, typecheck, format, generate, audit, and release tasks.
Language toolchains come from Bazel. No separate installs.

## Install

Bazel via Bazelisk (reads `.bazelversion`).

## Quickstart

`dx` runs through Bazel.

```sh
bazel run @rules_dx//:dx -- --help
bazel run @rules_dx//:dx -- build //...
bazel run @rules_dx//:dx -- test //...
```

`dx init --apply` writes the overrides `rules_dx` needs; see
[patches/README.md](patches/README.md).

Next: [docs index](docs/README.md),
[command reference](docs/cli/commands/README.md),
[examples](examples/README.md), [CI](docs/github-ci.md), and the
[docs site](https://devhusdk.github.io/rules_dx/).

## Contributing

See [Contributing](CONTRIBUTING.md) for the current workflow.

## License

`rules_dx` is licensed under [Apache-2.0](LICENSE).
