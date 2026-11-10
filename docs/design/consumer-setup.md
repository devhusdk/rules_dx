# Consumer setup design

This page fixes what a consumer writes to use rules_dx, and what rules_dx writes and carries for them. `dx init`, the root `MODULE.bazel`, the language extensions, the README and the release follow it.

## One command

- `dx init --apply` is the only setup command. It runs in an empty directory, a source tree with no Bazel files, and an existing Bazel repo.
- After it exits 0, `tools/dx test //...` passes with no file edited by hand.
- It writes `MODULE.bazel`, `.bazelversion`, `.bazelrc`, the root `BUILD.bazel` and the `tools/dx` launcher when they are missing.
- It detects languages from the `file_extensions` and `manifests` in each `<lang>/lang.json`.
- It runs `dx generate` as its last step. `dx generate` writes every `BUILD.bazel` and every missing lockfile.
- A second run changes no file.
- In an existing Bazel repo it adds what is missing. It never rewrites a stanza that names the same lockfile, SDK or toolchain.

## Consumer MODULE.bazel

- A consumer has one `bazel_dep`: `rules_dx`.
- A consumer writes no override, `override_repo`, `inject_repo`, `register_toolchains` or `use_repo_rule` for a rules_dx language.
- Every extension a consumer uses is in `@rules_dx//<lang>:extensions.bzl` or `@rules_dx//core:extensions.bzl`. No consumer names an upstream rule set.
- `dx init` writes two kinds of tag: a dependency hub and a license. A user writes any other tag to change a default.

## Dependency hubs

- For each dependency set of `locks.md`, `dx init` writes one hub tag and its `use_repo`.
- The hub tag takes the lockfile label. It takes no version, url or `sha256`.
- `<lang>/extensions.bzl` holds the hub extension.

## Patched dependencies

- The root `MODULE.bazel` has no override of any kind.
- A patched upstream that some module in the graph depends on is a version in the rules_dx registry.
- A patched upstream that no module depends on is an archive fetched by a repository rule in `<lang>/private/`, with its `patches`.
- The registry is `patches/`: `patches/bazel_registry.json` and `patches/modules/<module>/<version>/` with `MODULE.bazel`, `source.json` and `patches/`.
- A patched version is named `<upstream version>.dx.<n>`. `n` goes up by one when the patch set changes.
- An upstream release that the Bazel Central Registry lacks enters the registry the same way, with no patch.
- `source.json` holds the integrity of the archive and of every patch.
- rules_dx names the registry version in its `bazel_dep`. Root and consumer resolve the same files.
- Root `.bazelrc` sets `--registry=file://%workspace%/patches` ahead of the Bazel Central Registry.
- The release copies `patches/modules/` to the `registry` branch and adds the `rules_dx` version. A published version directory never changes.
- The consumer `.bazelrc` names `https://raw.githubusercontent.com/devhusdk/rules_dx/registry` ahead of the Bazel Central Registry.
- rules_dx goes to the Bazel Central Registry only when `patches/modules/` is empty.
- `dx check` fails when a module under `patches/modules/` resolves to another version in the consumer graph. It names the module that asked for it.

## Root module

- No `register_toolchains` or `register_execution_platforms` call is `dev_dependency`. A test that needs another toolchain selects it with a transition.
- A repo that only a `dev_dependency` usage brings in is named only under `tests/`, `tools/` and `examples/`. No `<lang>/tests/consumer/` names one.
- A `dev_dependency` tag never supplies a version, url, `sha256`, SDK part or target platform that a language rule needs.

## Defaults

- Every toolchain and SDK has a default version in `<lang>/pins.json`, with its urls and `sha256`.
- The extension that declares a toolchain or SDK declares the default repos when no module sets a tag. Bazel fetches a repo only when a build needs it.
- A consumer writes a tag only to pick another pinned version. A consumer never writes a `sha256`.
- An SDK with optional parts declares one repo per part. A rule attribute picks the parts. No tag lists parts.
- rules_dx registers every toolchain of every platform in the manifest `platforms`, cross targets included.
- rules_dx registers its SDK toolchains ahead of an upstream default, and patches an upstream that names a fixed repo.

## Licenses

- A license is the only setup a user must decide.
- One extension accepts licenses: `dx_licenses` in `core/extensions.bzl`, with the tag `accept(ids = [...])`. Only root-module tags count.
- No other extension, rule attribute or environment variable accepts a license.
- The manifest `setup.licenses` lists the ids a language needs and the platforms that need them.
- `dx init --accept-license <id>` writes the tag.
- When the detected languages need an id that is neither given nor in `MODULE.bazel`, `dx init` exits non-zero, names the id and the flag, and writes nothing.
- A fetch that needs an id that is not accepted fails and names the id.

## Flags

- No language needs a flag to build with its default toolchain. A flag only picks another registered toolchain or a mode.
- A rule that chooses what to build for takes `platforms` and applies a transition. No consumer sets `--platforms`, a minimum OS flag or an SDK version flag.
- A flag Bazel can take only from an rc file is in the manifest `setup.flags`. dx writes it into `.dx/bazelrc/` when the workspace has that language's files.
- The `.bazelrc` that `dx init` writes holds the two registry lines and the `try-import` lines of `.dx/bazelrc/`. It holds no other flag.
- Root `.bazelrc` gets language flags from the same generated layers. It sets no other build flag, except the names in `tools/ci/root_bazelrc_allowlist.txt`. That list only shrinks.

## Platform families

What a consumer writes beyond its sources:

| Family | `MODULE.bazel` | Per target |
| --- | --- | --- |
| Apple | `apple-xcode-license` | `bundle_id`, signing attributes |
| Android | `android-sdk-license` | `app_id`, signing attributes |
| CUDA | `cuda-eula` | `cuda_archs` |
| Qt | nothing | `modules` |
| MSIX | `windows-sdk-buildtools` | identity and signing attributes |
| C and C++ with MSVC | `visual-studio-build-tools` | nothing |
| Every other language | nothing | nothing |

- An app macro declares its own provisioning target. One call builds one app.

## Manifest

- `setup` is a field of `<lang>/lang.json`. Its keys are `licenses`, `flags` and `hubs`.
- `hubs` holds the ecosystem entries that `locks.md` fixes.
- `dx init` reads `setup`. No Rust file repeats an extension label, a tag name or a rule-set version.

## Docs

- The README quickstart is: get the `dx` release binary, run `dx init --apply`. It shows no file to create.
- A generated language page shows the consumer `MODULE.bazel` of its fixture.
- No user page shows an override, a `sha256` or a toolchain registration.

## Tests

- Every language with `entry_points` has `<lang>/tests/consumer/`: the smallest sources that build one target and run one test, with no third-party package.
- The directory holds the sources and the files `dx init --apply` writes for them: `BUILD.bazel`, and `MODULE.bazel` stored as `consumer.MODULE.bazel`.
- Its `BUILD.bazel` is a package of the main module and loads `@rules_dx//<lang>:defs.bzl`. Its targets are in-graph fixtures in the default set.
- One unit test per language copies the sources under `TEST_TMPDIR`, runs `dx init --apply` with a fake Bazel and the Gazelle binary from runfiles, and compares every written file with the checked-in one.
- One unit test reads the `MODULE.bazel` snapshot and fails on an override, a `dev_dependency` toolchain registration, or a dev-only repo named outside the allowed directories.
- It fails when a `consumer.MODULE.bazel` holds anything but `module`, the `rules_dx` `bazel_dep` and license tags.
- These tests start no Bazel server.
- `tools/testing/consumer/pending.txt` lists the languages with no passing fixture. It only shrinks.
- `//deploy/private/release/tests:release_consumer_smoke_test` is the one nested proof of module resolution. It runs the README commands with no text replaced.
