# rules_dx

Pre-release. `rules_dx` is a Bazel developer platform with one `dx` CLI for
build, test, lint, typecheck, format, generate, audit, and release tasks.
Language toolchains come from Bazel. No separate installs.

## Install

Prerequisites: Bazel via Bazelisk (see `.bazelversion`).

```sh
bazel build //...
```

## Quickstart

`dx` runs through Bazel. No install step publishes `dx` outside the repo yet.

```sh
bazel run //cli/cli:dx -- --help
bazel run //cli/cli:dx -- build //...
bazel run //cli/cli:dx -- test //...
```

Next: [Command reference](docs/cli/commands/README.md),
[examples](examples/README.md), [CI](docs/github-ci.md), and the
[docs site](https://ralvik.github.io/rules_dx/).

## Contributing

See [Contributing](CONTRIBUTING.md) for the current workflow.

## License

`rules_dx` is licensed under [Apache-2.0](LICENSE).
