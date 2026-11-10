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

The Gazelle binary behind `dx generate` works the same way. Each release
ships a pure-Go `dx-gazelle-<os>-<cpu>` per platform, pinned in
`GAZELLE_SHA256` under the same `DX_VERSION`, so the first `dx generate`
fetches no Go SDK. It builds from source instead when `rules_dx` is the root
module, when no gazelle binary is pinned, or when a module other than
`rules_dx` adds a `dx_gazelle.language`, because the release binary holds only
the languages `rules_dx` registers. The root module can override it:

```starlark
dx_gazelle = use_extension("@rules_dx//gazelle/dispatch:extension.bzl", "dx_gazelle")
dx_gazelle.asset(name = "dx-gazelle-linux-x86_64", url = "https://...", sha256 = "...")
```

or build it with `dx_gazelle.from_source()`, which needs a Go toolchain.

A global `dx` shim for PATH is built from `//cli/shim`. It runs the version in
`.dx/pins` without Bazel. See
[Global Shim](docs/cli/commands/README.md#global-shim).

## C and C++ toolchains

- Microsoft `cl.exe` (`--//cc:compiler=msvc`): the Visual Studio Build Tools come from the hermetic `@dx_msvc` repo, not an installed Visual Studio. The download needs the Visual Studio Build Tools license, accepted with `msvc.license(accept = ["visual-studio-build-tools"])`. Qualified on `windows_x86_64`: [run 42188542802](https://github.com/devhusdk/rules_dx/actions/runs/42188542802) passed `msvc_spaced_test` and `msvc_shared_test` with `_MSC_FULL_VER=195035728`, and [run 42171584868](https://github.com/devhusdk/rules_dx/actions/runs/42171584868) passed `msvc_hello_test`. The hermetic harness hides the preinstalled Visual Studio and Windows Kits from `c_fixture_msvc` and `cpp_fixture_msvc`, which qualified on `windows_x86_64` at e76d6f5dd in ci.yml [run 42372644803](https://github.com/devhusdk/rules_dx/actions/runs/42372644803). On `windows_arm64` they qualified at 2660a4b8d in ci.yml [run 42375312749](https://github.com/devhusdk/rules_dx/actions/runs/42375312749), and `msvc_arm64_hello_test` and `msvc_arm64_pe_machine_test` passed in [run 42183869292](https://github.com/devhusdk/rules_dx/actions/runs/42183869292). See the [support matrix](docs/support-matrix.md).
- GCC (`--//cc:compiler=gcc`): `c_fixture_gcc` and `cpp_fixture_gcc` qualified with `GCC: (GNU) 16.2.0` on native `linux_x86_64` and `linux_arm64` at e76d6f5dd in ci.yml [run 42372644803](https://github.com/devhusdk/rules_dx/actions/runs/42372644803), with no install and the network denied.

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

## Contributing

See [Contributing](CONTRIBUTING.md) for the current workflow.

## License

`rules_dx` is licensed under [Apache-2.0](LICENSE).
