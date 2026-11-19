# rules_dx

`rules_dx` gives you one `dx` command to build, test, lint, typecheck,
format, generate, audit and release code in many languages. It runs on
Bazel, a build tool that caches every step, and it fetches each language's
toolchain for you, so you install nothing per language.

## Quickstart

You need Bazelisk, the launcher that downloads the right Bazel version, on
your `PATH` as `bazel`. Download `dx-<os>-<cpu>` for your platform
(`.exe` on Windows) from the
[latest release](https://github.com/devhusdk/rules_dx/releases/latest) and
put it on your `PATH` as `dx`.

In an empty directory, set the project up and run its tests:

```sh
dx init --apply
tools/dx test //...
```

On Windows, set it up with the [Visual Studio Build Tools license](https://visualstudio.microsoft.com/license-terms/) accepted instead:

```sh
dx init --apply --accept-license visual-studio-build-tools
```

`dx init --apply` writes `MODULE.bazel`, which depends on `rules_dx`,
`.bazelversion`, the `tools/dx` launcher, CI and editor files. `//...` means
every target in the project. From now on, run `dx` through the launcher:

```sh
tools/dx --help
tools/dx build //...
```

On a fresh clone, run `tools/dx` once first. Any `dx` command writes the
gitignored `.dx/bazelrc` files that plain `bazel` reads.

Behind a proxy or a company CA? See [Corporate proxy and custom CA](docs/proxy-and-ca.md).

## Next steps

1. Follow the [tutorial](docs/tutorial.md). It takes you from an empty
   directory to two tested packages and `dx test --affected`.
2. Read [Bazel for dx users](docs/bazel-for-dx-users.md) for the Bazel terms
   and messages you meet in your first hour.
3. Find your task in the [docs index](docs/README.md), or look up a command
   in the [command reference](docs/cli/commands/README.md).

Already have a Bazel repo? Read
[adopting rules_dx in an existing Bazel repo](docs/adopting-existing-bazel.md).
New to Bazel and moving an existing project? Read
[adopting rules_dx without Bazel experience](docs/adopting-without-bazel.md).
You can also browse the [examples](examples/README.md), set up
[CI](docs/github-ci.md), check the [support matrix](docs/support-matrix.md),
or read everything on the [docs site](https://devhusdk.github.io/rules_dx/).

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

## Which dx binary runs

In your module, `@rules_dx//:dx` runs the `dx` binary for your platform. It
is checked against the sha256 in `cli/prebuilt/pins.bzl`. A release pins
binaries for linux x86_64, linux aarch64, macos aarch64, windows x86_64 and
windows aarch64.

- A version with no pinned sha256 builds `dx` from source, with the Rust and
  C toolchains `dx init` sets up.
- Once some binary is pinned, a platform with no release binary fails the
  build.

To pick a binary yourself, add this to your root `MODULE.bazel`:

```starlark
dx_prebuilt = use_extension("@rules_dx//core:extensions.bzl", "dx_prebuilt")
dx_prebuilt.asset(name = "dx", os = "linux", cpu = "x86_64", url = "https://...", sha256 = "...")
```

`name` is the tool, and `os` and `cpu` are the `@platforms` values the binary
runs on. A jar tool such as `javac_check` takes `os = "any"` and
`cpu = "any"`. Or build `dx` from source with `dx_prebuilt.from_source()`. That needs
Rust and C toolchains. Only the root module's tags count.

`cli/prebuilt/tools.bzl` lists every tool a release ships prebuilt, and
`pins.bzl` holds their sha256s under `PINS`, keyed by tool and then by
`<os>_<cpu>`.

The Gazelle binary behind `dx generate` works the same way. Each release
ships a pure-Go `dx-gazelle-<os>-<cpu>` per platform, pinned under
`PINS["gazelle"]` with the same `DX_VERSION`, so the first `dx generate`
fetches no Go SDK. It builds from source instead when:

- `rules_dx` is the root module,
- no gazelle binary is pinned, or
- a module other than `rules_dx` adds a `dx_gazelle.language`, because the
  release binary holds only the languages `rules_dx` registers.

The root module can override it:

```starlark
dx_gazelle = use_extension("@rules_dx//core:extensions.bzl", "dx_gazelle")
dx_gazelle.asset(name = "gazelle", os = "linux", cpu = "x86_64", url = "https://...", sha256 = "...")
```

or build it with `dx_gazelle.from_source()`, which needs a Go toolchain.

A global `dx` shim for your `PATH` is built from `//cli/shim`. It runs the
version in `.dx/pins` without Bazel. See
[Global Shim](docs/cli/commands/README.md#global-shim).

## C and C++ toolchains

- Microsoft `cl.exe` (`--//cc:compiler=msvc`): the Visual Studio Build Tools come from the hermetic `@dx_msvc` repo, not an installed Visual Studio. The download needs the Visual Studio Build Tools license, accepted with `dx_licenses.accept(ids = ["visual-studio-build-tools"])`. See the [support matrix](docs/support-matrix.md).
- GCC (`--//cc:compiler=gcc`): GCC 16.2.0 on `linux_x86_64` and `linux_arm64`, with no install.

## Contributing

See [Contributing](CONTRIBUTING.md) for the current workflow.

## License

`rules_dx` is licensed under [Apache-2.0](LICENSE).
