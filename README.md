# rules_dx

`rules_dx` is a Bazel developer platform with one `dx` CLI for
build, test, lint, typecheck, format, generate, audit, and release tasks.
Language toolchains come from Bazel. No separate installs.

Status: rules_dx is not released yet. Install starts with the first BCR release.

## Quickstart

Bazel via Bazelisk is required. In an empty directory, create three files.

`.bazelversion`:

```text
9.2.0
```

`MODULE.bazel`:

```starlark
bazel_dep(name = "rules_dx", version = "<release>")
```

`BUILD.bazel`:

```starlark
# Empty on purpose: rules_rust crate_universe needs a root package.
```

Then:

```sh
bazel run @rules_dx//:dx -- init --apply
tools/dx test //...
```

After that, run `dx` through the launcher:

```sh
tools/dx --help
tools/dx build //...
```

On a fresh clone, run `tools/dx` first. Any dx command writes the gitignored `.dx/bazelrc` layers that plain `bazel` reads.

Behind a proxy or a company CA? See [Corporate proxy and custom CA](docs/proxy-and-ca.md).

## Which dx binary runs

In your module, `@rules_dx//:dx` will run the `dx` binary for the target
platform, checked against the sha256 in `cli/prebuilt/pins.bzl`.
Release binaries are planned for linux x86_64, linux aarch64, macos aarch64,
windows x86_64 and windows aarch64. Until a release ships, a version with no
pinned sha256 builds `dx` from source, with the Rust and C toolchains
`dx init` sets up. Once some binary is pinned, platforms with no release
binary fail the build. To pick a binary yourself, add this to your root
`MODULE.bazel`:

```starlark
dx_prebuilt = use_extension("@rules_dx//cli/prebuilt:extension.bzl", "dx_prebuilt")
dx_prebuilt.asset(name = "dx-linux-x86_64", url = "https://...", sha256 = "...")
```

Or build `dx` from source with `dx_prebuilt.from_source()`. That needs Rust
and C toolchains. Only the root module's tags count.

`dx init --apply` writes the overrides `rules_dx` needs; see
[patches/README.md](patches/README.md).

A global `dx` shim for PATH is built from `//cli/shim`. It runs the version in
`.dx/pins` without Bazel. See
[Global Shim](docs/cli/commands/README.md#global-shim).

## C and C++ toolchains

- Microsoft `cl.exe` (`--//cc/toolchains:compiler=msvc`): the Visual Studio Build Tools come from the hermetic `@dx_msvc` repo, not an installed Visual Studio. The download needs the Visual Studio Build Tools license, accepted with `msvc.license(accept = ["visual-studio-build-tools"])`. Qualified on `windows_x86_64` only: [run 42171584868](https://github.com/devhusdk/rules_dx/actions/runs/42171584868) passed `msvc_hello_test` and `msvc_spaced_test`, but `msvc_shared_test` still fails to link (LNK2019, #5954). No `_MSC_FULL_VER` is recorded yet. `windows_arm64` has no passing run. See the [support matrix](docs/support-matrix.md).

## Next

[tutorial](docs/tutorial.md), [docs index](docs/README.md),
[Bazel for dx users](docs/bazel-for-dx-users.md),
[adopting rules_dx in an existing Bazel repo](docs/adopting-existing-bazel.md),
[adopting rules_dx without Bazel experience](docs/adopting-without-bazel.md),
[command reference](docs/cli/commands/README.md),
[examples](examples/README.md), [CI](docs/github-ci.md),
[support matrix](docs/support-matrix.md),
and the
[docs site](https://devhusdk.github.io/rules_dx/).

## What rules_dx replaces

You get:

- One [`dx` CLI](docs/cli/commands/README.md) instead of per-language scripts.
- Pinned toolchains, so nobody installs tools per machine.
- Lint, format, and typecheck locally and in [CI](docs/github-ci.md).
- Security and license audits with `dx security` and `dx license`; see [audit and update](docs/cli/commands/audit-update.md).
- A CI workflow from `dx init --apply`; see [init](docs/cli/commands/init.md).
- It replaces hand-picked rule-set versions, a Makefile or shell wrapper around Bazel, and CI config per language.

It costs:

- Bazel and its learning curve: BUILD files and `MODULE.bazel`.
- The files `dx init` writes: launcher, editor, devcontainer, CI and agent files; see [init](docs/cli/commands/init.md).
- Following the pinned rule and tool versions. You move when you run [`dx upgrade`](docs/cli/commands/upgrade.md).
- Patched upstream overrides that `dx init --apply` writes to `third_party/rules_dx/`; see [rules_dx overrides](docs/cli/commands/init.md#rules_dx-overrides) and [patches/README.md](patches/README.md).

## Contributing

See [Contributing](CONTRIBUTING.md) for the current workflow.

## License

`rules_dx` is licensed under [Apache-2.0](LICENSE).
