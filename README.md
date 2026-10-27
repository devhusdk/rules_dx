# rules_dx

`rules_dx` is a Bazel developer platform with one `dx` CLI for
build, test, lint, typecheck, format, generate, audit, and release tasks.
Language toolchains come from Bazel. No separate installs.

## Install

Bazel via Bazelisk (reads `.bazelversion`).

A global `dx` shim for PATH is built from `//cli/shim`. It runs the version in
`.dx/version` without Bazel. See
[Global Shim](docs/cli/commands/README.md#global-shim).

## Quickstart

`dx` runs through Bazel.

```sh
bazel run @rules_dx//:dx -- --help
bazel run @rules_dx//:dx -- build //...
bazel run @rules_dx//:dx -- test //...
```

In your module, `@rules_dx//:dx` runs the released `dx` binary for the
target platform, checked against the sha256 in `cli/prebuilt/pins.bzl`.
Release binaries exist for linux x86_64, linux aarch64, macos aarch64,
windows x86_64 and windows aarch64. A platform whose binary has no pinned
sha256 builds `dx` from source instead, so a rules_dx version with no pins
at all (any version taken from git) builds `dx` from source everywhere, with
the Rust and C toolchains `dx init` sets up. Once some binary is pinned,
platforms with no release binary fail the build. To pick a binary yourself,
add this to your root `MODULE.bazel`:

```starlark
dx_prebuilt = use_extension("@rules_dx//cli/prebuilt:extension.bzl", "dx_prebuilt")
dx_prebuilt.asset(name = "dx-linux-x86_64", url = "https://...", sha256 = "...")
```

Or build `dx` from source with `dx_prebuilt.from_source()`. That needs Rust
and C toolchains. Only the root module's tags count.

`dx init --apply` writes the overrides `rules_dx` needs; see
[patches/README.md](patches/README.md).

Next: [docs index](docs/README.md),
[adopting rules_dx in an existing Bazel repo](docs/adopting-existing-bazel.md),
[command reference](docs/cli/commands/README.md),
[examples](examples/README.md), [CI](docs/github-ci.md), and the
[docs site](https://devhusdk.github.io/rules_dx/).

## Contributing

See [Contributing](CONTRIBUTING.md) for the current workflow.

## License

`rules_dx` is licensed under [Apache-2.0](LICENSE).
