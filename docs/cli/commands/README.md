# Command Reference

Put `dx` flags after the command, Bazel flags after `--`.

```sh
bazel run @rules_dx//:dx -- build //...
bazel run @rules_dx//:dx -- lint //... -- --jobs=4
```

- [`dx build`](build-test-coverage.md): build targets.
- [`dx test`](build-test-coverage.md): run tests.
- [`dx coverage`](build-test-coverage.md): collect LCOV coverage.
- [`dx run`](build-test-coverage.md): build and run a target.
- [`dx deploy`](build-test-coverage.md): build and run a deployable target.
- [`dx lint`](quality.md): run linters.
- [`dx typecheck`](quality.md): run type checkers.
- [`dx format`](quality.md): check or rewrite formatting.
- [`dx check`](check-fix-clean.md): run generate, format, lint and typecheck.
- [`dx clean`](check-fix-clean.md): remove build outputs.
- [`dx generate`](generate.md): update BUILD files with Gazelle.
- [`dx env`](env.md): set up the toolchain environment.
- [`dx security`](audit-update.md): run the security audit.
- [`dx license`](audit-update.md): run the license audit.
- [`dx update`](audit-update.md): update dependencies.
- [`dx docs`](docs.md): build and serve the docs site.
- [`dx explain`](explain.md): explain cache misses.
- [`dx init`](init.md): set up a workspace.
- [`dx status`](status.md): report workspace status.
- [`dx completion`](completion.md): print shell completions.
- [`dx upgrade`](upgrade.md): move the dx pin.
- [`dx watch`](watch.md): rebuild on changes.
- [`dx worktree`](worktree.md): lease warm checkout slots for agents.

Scope rules: [Scope Defaults](scope-defaults.md). Airgapped hosts:
[Offline Bundle](offline-bundle.md).

## Global Flags

Every command takes these.

| Flag | What it does |
| --- | --- |
| `-h`, `--help` | Print help. |
| `-V`, `--version` | Print the dx version. |
| `--workspace <dir>` | Run in another workspace. |
| `-q`, `--quiet[=bool]` | Less output. |
| `-v`, `--verbose[=bool]` | More output. |
| `--color auto\|always\|never` | Color output. Default `auto`. |
| `--build-slots auto\|<n>\|off` | Build slots for this run. Default `auto`. |
| `--bazel-startup-option <token>` | Bazel startup option. Repeatable. Takes `--output_base=<path>` or `--output_user_root=<path>`. |

Other flags belong to one command family. `dx <command> --help` lists them.

| Family | Flags |
| --- | --- |
| Read commands | `-o`, `--output text\|json`. Commands that can print a patch also take `diff`. |
| Check commands | `--fail-on info\|warning\|error` (default `warning`), `--report <format>=<dest>` (repeatable). |
| Coverage | `--min-coverage <percent>`. |
| Network commands | `--offline` (no network), `--frozen` (no manifest or lock changes). |
| Scoped commands | `--here` (current directory tree), `--affected` (targets your changes reach). |
| Mutating commands | `--apply`. |
| Build commands | `--debug`, `--release`. |

A relative `--report` destination starts at the workspace root. Parent
directories must exist.

## Output Values

`--output text` prints for people. `--output json` prints one JSON object per
line on stdout. Every object has an `event` name and a `schema` with `major`
and `minor`. The stream starts with `command_started` and ends with
`command_finished`. Logs go to stderr. The schema is
[`cli/output/schema/events.schema.json`](../../../cli/output/schema/events.schema.json).

```sh
bazel run @rules_dx//:dx -- lint //... --output=json
```

When `CLAUDECODE`, `CODEX_THREAD_ID`, `CURSOR_AGENT`, `GEMINI_CLI` or
`AI_AGENT` is set, the default output is `json`. See
[Agent sessions](worktree.md#agent-sessions).

## Exit Codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | Failed check or operational failure. |
| `2` | Usage error. |
| Bazel's code | Bazel failed. |
| `101` | dx crashed. |
| `128` plus signal | A signal stopped dx. |

## Build Slots

Commands that run Bazel wait for a build slot, so parallel runs on one machine
do not run out of memory. Slots are shared by every worktree on the machine.
The count is the machine memory, or the cgroup limit if lower, divided by 3 GiB.
It is at least 1 and at most 16.

```sh
bazel run @rules_dx//:dx -- build //... --build-slots 4
```

- `auto`: the memory-based count. This is the default inside a
  `dx worktree` slot.
- `<n>`: exactly `n` slots, from 1 to 1024.
- `off`: take no slot. This is the default outside a `dx worktree` slot.

`DX_BUILD_SLOTS` sets a default. The flag wins. A waiting run prints
`dx: waiting for a build slot` to stderr and runs when a slot frees. Run
`dx status` to see who holds the slots. Ctrl-C ends the wait.

With a Bazel `--remote_executor`, dx offers more slots: the memory divided by
1.5 GiB. A run takes an extra slot only when no regular slot is free and the
host is not under memory pressure. On Linux that means `/proc/pressure/memory`
`some avg10` is under 10. On macOS it means the kernel level
`kern.memorystatus_vm_pressure_level` is 1.

While a slot is held, dx adds `--local_resources=cpu=<n>` and
`--local_resources=memory=<mb>` to `build`, `test`, `coverage` and `run`. dx
skips them when an rc file or your Bazel arguments set the resource.

## Disk Space

Before each Bazel build, dx checks free space on the workspace, the Bazel output
base, the temp dir and the home dir. Under 1 GiB free, it prints a `disk_low`
line to stderr and keeps going. Free space with:

```sh
bazel run @rules_dx//:dx -- worktree prune --apply
```

## Environment

A flag on the command line wins over these defaults.

| Variable | What it does |
| --- | --- |
| `DX_WORKSPACE=<dir>` | Default for `--workspace`. |
| `DX_QUIET=<bool>` | Default for `--quiet`. |
| `DX_VERBOSE=<bool>` | Default for `--verbose`. |
| `DX_COLOR=<mode>` | Default for `--color`. |
| `DX_OUTPUT=<mode>` | Default for `--output`. |
| `DX_FAIL_ON=<level>` | Default for `--fail-on`. |
| `DX_BUILD_SLOTS=auto\|<n>\|off` | Default for `--build-slots`. |
| `NO_COLOR=<any>` | Disable color in `auto` mode. |
| `BUILD_WORKSPACE_DIRECTORY=<dir>` | Workspace start under `bazel run`. |
| `BUILD_WORKING_DIRECTORY=<dir>` | Current directory under `bazel run`, for `--here` and a relative `worktree release <path>`. |

A `<bool>` is `true` or `false`. Any other non-empty value is a usage error. In
CI the `DX_` variables are ignored. Flags still win.

## Global Shim

The shim is a small `dx` binary for PATH, built from `//cli/shim`. It runs the
dx version a workspace pins without Bazel and never edits the workspace.

1. The version is `DX_VERSION` when set, otherwise the version of the single
   one-line `bazel_dep(name = "rules_dx", version = "X")` in the nearest
   `MODULE.bazel` above the current directory. It must be plain semver.
2. The binary is `<cache>/bin/<version>/dx-<os>-<arch>` (`.exe` on Windows),
   the same file `tools/dx` uses. A cached binary runs with no network access.
3. A missing binary is downloaded from the GitHub release and checked against
   the `<version> <sha256> <asset>` line in the `.dx/pins` next to that
   `MODULE.bazel`. A binary that does not match is deleted and never run.
4. Without a pinned hash for the version (no `bazel_dep` line, a multi-line or
   non-literal dep, any `module_name = "rules_dx"` override, no `.dx/pins`
   line, or no `MODULE.bazel`), the shim runs `bazel run @rules_dx//:dx -- ...`
   and prints nothing itself. It never falls back to the release `.sha256`.
5. The shim runs the binary with the same arguments, stdin, stdout and stderr,
   and sets `DX_SHIM=1`.

`dx shim version` prints `dx-shim <version>`. `dx shim where` prints the
version, where it came from (`DX_VERSION`, `MODULE.bazel` or `bazel`), the binary path, and whether it is cached. It
never downloads. The dx binary rejects `shim` as a command name.

Environment:

- `DX_VERSION=<semver>`: run this version instead of the `MODULE.bazel` version. This bootstrap trusts the release
  `.sha256` file.
- `DX_CACHE_DIR=<dir>`: cache root. Otherwise `$XDG_CACHE_HOME/dx`, then
  `~/.cache/dx` (`%USERPROFILE%\.cache\dx` on Windows). Empty values are unset.
- `DX_RELEASE_BASE_URL=<url>`: download from `<url>/v<version>/<asset>`
  instead of the GitHub release. Must be `https://`, or `http://` to
  `127.0.0.1`, `[::1]` or `localhost`. With `DX_VERSION` the shim trusts this server and its
  `.sha256` files fully.

Errors print one `dx: ` line on stderr. Exit codes: `0` success, `2` usage
errors (bad `DX_VERSION`, bad `DX_RELEASE_BASE_URL`, unknown `dx shim`
command), `1` other failures (no `.dx/pins`, no binary for the platform,
download failure, sha256 mismatch, cache not writable). Otherwise the exit code
or signal is the dx binary's own.

## Supported Surface

`@rules_dx//:dx` runs every command.

Language rules:

- `@rules_dx_android//rules:defs.bzl`: Android libraries.
- `@rules_dx//api_spec/rules:defs.bzl`: OpenAPI and AsyncAPI specs.
- `@rules_dx//apple/ios:app.bzl`: iOS apps, tests and frameworks.
- `@rules_dx//apple/ios:defs.bzl`: iOS libraries.
- `@rules_dx//apple/macos:app.bzl`: macOS apps.
- `@rules_dx//cc/rules:defs.bzl`: C and C++.
- `@rules_dx//csharp/rules:defs.bzl`: C#.
- `@rules_dx//cuda/rules:defs.bzl`: CUDA.
- `@rules_dx//dockerfile/rules:defs.bzl`: Dockerfiles.
- `@rules_dx//make/rules:defs.bzl`: Makefiles.
- `@rules_dx//cmake/rules:defs.bzl`: CMake files.
- `@rules_dx//d/rules:defs.bzl`: D.
- `@rules_dx//elixir/rules:defs.bzl`: Elixir.
- `@rules_dx//erlang/rules:defs.bzl`: Erlang.
- `@rules_dx//fsharp/rules:defs.bzl`: F#.
- `@rules_dx//go/rules:defs.bzl`: Go.
- `@rules_dx//haskell/rules:defs.bzl`: Haskell.
- `@rules_dx//java/rules:defs.bzl`: Java.
- `@rules_dx//javascript/rules:defs.bzl`: JavaScript.
- `@rules_dx//javascript/rules:web.bzl`: web apps and dev servers.
- `@rules_dx//ansible/rules:defs.bzl`: Ansible.
- `@rules_dx//bicep/rules:defs.bzl`: Bicep.
- `@rules_dx//cloudformation/rules:defs.bzl`: CloudFormation.
- `@rules_dx//julia/rules:defs.bzl`: Julia.
- `@rules_dx//kotlin/rules:defs.bzl`: Kotlin.
- `@rules_dx//lua/rules:defs.bzl`: Lua.
- `@rules_dx//kubernetes/rules:defs.bzl`: Kubernetes.
- `@rules_dx//perl/rules:defs.bzl`: Perl.
- `@rules_dx//php/rules:defs.bzl`: PHP.
- `@rules_dx//powershell/rules:defs.bzl`: PowerShell.
- `@rules_dx//python/rules:defs.bzl`: Python.
- `@rules_dx//qt/rules:defs.bzl`: Qt.
- `@rules_dx//qt/rules:rust.bzl`: Rust libraries for Qt.
- `@rules_dx//quality/rules:defs.bzl`: CUE, Gherkin, HTML templates, Jsonnet, Jupyter, LaTeX, Nix, Pkl, Typst, XML, YAML.
- `@rules_dx//rego/rules:defs.bzl`: Rego.
- `@rules_dx//ruby/rules:defs.bzl`: Ruby.
- `@rules_dx//rust/rules:defs.bzl`: Rust.
- `@rules_dx//scala/rules:defs.bzl`: Scala.
- `@rules_dx//sql/rules:defs.bzl`: SQL.
- `@rules_dx//terraform/rules:defs.bzl`: Terraform.
- `@rules_dx//typescript/rules:defs.bzl`: TypeScript.
- `@rules_dx//vue/rules:defs.bzl`: Vue.
- `@rules_dx//zig/rules:defs.bzl`: Zig.

### Rule names

dx names rules `<language>_<kind>`. This table maps each upstream rule to the
dx rule that wraps it.

| Upstream load | Upstream rule | dx load | dx rule |
| --- | --- | --- | --- |
| `@rules_android//rules:rules.bzl` | `android_library` | `@rules_dx_android//rules:defs.bzl` | `java_android_library` |
| `@rules_kotlin//kotlin:android.bzl` | `kt_android_library` | `@rules_dx_android//rules:defs.bzl` | `kotlin_android_library` |
| `@rules_apple//apple:ios.bzl` | `ios_application` | `@rules_dx//apple/ios:app.bzl` | `apple_ios_app` |
| `@rules_apple//apple:ios.bzl` | `ios_unit_test` | `@rules_dx//apple/ios:app.bzl` | `apple_ios_app_test` |
| `@rules_apple//apple:apple.bzl` | `apple_xcframework` | `@rules_dx//apple/ios:app.bzl` | `apple_ios_xcframework` |
| `@rules_apple//apple:macos.bzl` | `macos_application` | `@rules_dx//apple/macos:app.bzl` | `apple_macos_app` |
| `@rules_cc//cc:defs.bzl` | `cc_binary`, `cc_library`, `cc_test` | `@rules_dx//cc/rules:defs.bzl` | `cc_binary`, `cc_library`, `cc_test` |
| `@rules_cc//cc:objc_library.bzl` | `objc_library` | `@rules_dx//cc/rules:defs.bzl` | `objc_library` |
| `@rules_dotnet//dotnet:defs.bzl` | `csharp_binary`, `csharp_library`, `csharp_test` | `@rules_dx//csharp/rules:defs.bzl` | `csharp_binary`, `csharp_library`, `csharp_test` |
| `@rules_cuda//cuda:defs.bzl` | `cuda_library`, `cuda_objects` | `@rules_dx//cuda/rules:defs.bzl` | `cuda_library`, `cuda_objects` |
| `@rules_d//d:defs.bzl` | `d_binary`, `d_library`, `d_test` | `@rules_dx//d/rules:defs.bzl` | `d_binary`, `d_library`, `d_test` |
| `@rules_elixir//:elixir_app.bzl` | `elixir_app` | `@rules_dx//elixir/rules:defs.bzl` | `elixir_app` |
| `@rules_elixir//:ex_unit_test.bzl` | `ex_unit_test` | `@rules_dx//elixir/rules:defs.bzl` | `ex_unit_test` |
| `@rules_erlang//:erlang_app.bzl` | `erlang_app`, `test_erlang_app` | `@rules_dx//erlang/rules:defs.bzl` | `erlang_app`, `test_erlang_app` |
| `@rules_erlang//:ct.bzl` | `ct_suite` | `@rules_dx//erlang/rules:defs.bzl` | `ct_suite` |
| `@rules_erlang//:erlang_bytecode2.bzl` | `erlang_bytecode` | `@rules_dx//erlang/rules:defs.bzl` | `erlang_bytecode` |
| `@rules_erlang//:escript.bzl` | `escript_archive` | `@rules_dx//erlang/rules:defs.bzl` | `escript_archive` |
| `@rules_erlang//:eunit2.bzl` | `eunit` | `@rules_dx//erlang/rules:defs.bzl` | `eunit` |
| `@rules_dotnet//dotnet:defs.bzl` | `fsharp_binary`, `fsharp_library`, `fsharp_test` | `@rules_dx//fsharp/rules:defs.bzl` | `fsharp_binary`, `fsharp_library`, `fsharp_test` |
| `@rules_go//go:def.bzl` | `go_binary`, `go_library`, `go_test` | `@rules_dx//go/rules:defs.bzl` | `go_binary`, `go_library`, `go_test` |
| `@rules_haskell//haskell:defs.bzl` | `haskell_binary`, `haskell_library`, `haskell_test` | `@rules_dx//haskell/rules:defs.bzl` | `haskell_binary`, `haskell_library`, `haskell_test` |
| `@rules_java//java:defs.bzl` | `java_binary`, `java_library`, `java_test` | `@rules_dx//java/rules:defs.bzl` | `java_binary`, `java_library`, `java_test` |
| `@aspect_rules_js//js:defs.bzl` | `js_binary` | `@rules_dx//javascript/rules:defs.bzl` | `javascript_binary` |
| `@aspect_rules_js//js:defs.bzl` | `js_library` | `@rules_dx//javascript/rules:defs.bzl` | `javascript_library` |
| `@aspect_rules_js//js:defs.bzl` | `js_test` | `@rules_dx//javascript/rules:defs.bzl` | `javascript_test` |
| `@aspect_rules_jest//jest:defs.bzl` | `jest_test` | `@rules_dx//javascript/rules:defs.bzl` | `javascript_test` |
| `@rules_julia//julia:defs.bzl` | `julia_binary`, `julia_library`, `julia_test` | `@rules_dx//julia/rules:defs.bzl` | `julia_binary`, `julia_library`, `julia_test` |
| `@rules_kotlin//kotlin:jvm.bzl` | `kt_jvm_binary`, `kt_jvm_library`, `kt_jvm_test` | `@rules_dx//kotlin/rules:defs.bzl` | `kotlin_binary`, `kotlin_library`, `kotlin_test` |
| `@rules_perl//perl:defs.bzl` | `perl_binary`, `perl_library`, `perl_test` | `@rules_dx//perl/rules:defs.bzl` | `perl_binary`, `perl_library`, `perl_test` |
| `@rules_powershell//powershell:defs.bzl` | `pwsh_binary`, `pwsh_library`, `pwsh_test` | `@rules_dx//powershell/rules:defs.bzl` | `pwsh_binary`, `pwsh_library`, `pwsh_test` |
| `@rules_py//py:defs.bzl` | `py_binary` | `@rules_dx//python/rules:defs.bzl` | `python_binary` |
| `@rules_py//py:defs.bzl` | `py_library` | `@rules_dx//python/rules:defs.bzl` | `python_library` |
| `@rules_py//py:defs.bzl` | `py_test`, `py_pytest_test` | `@rules_dx//python/rules:defs.bzl` | `python_test` |
| `@rules_ruby//ruby:defs.bzl` | `rb_binary`, `rb_library`, `rb_test` | `@rules_dx//ruby/rules:defs.bzl` | `ruby_binary`, `ruby_library`, `ruby_test` |
| `@rules_rust//rust:defs.bzl` | `rust_binary`, `rust_clippy_test`, `rust_library`, `rust_proc_macro`, `rust_shared_library`, `rust_static_library`, `rust_test`, `rustfmt_test` | `@rules_dx//rust/rules:defs.bzl` | same names |
| `@rules_scala//scala:scala.bzl` | `scala_binary`, `scala_library`, `scala_test` | `@rules_dx//scala/rules:defs.bzl` | `scala_binary`, `scala_library`, `scala_test` |
| `@aspect_rules_ts//ts:defs.bzl` | `ts_project` | `@rules_dx//typescript/rules:defs.bzl` | `typescript_project` |
| `@aspect_rules_js//js:defs.bzl` | `js_test` | `@rules_dx//typescript/rules:defs.bzl` | `typescript_test` |
| `@aspect_rules_jest//jest:defs.bzl` | `jest_test` | `@rules_dx//typescript/rules:defs.bzl` | `typescript_test` |
| `@aspect_rules_js//js:defs.bzl` | `js_library` | `@rules_dx//vue/rules:defs.bzl` | `vue_library` |
| `@rules_zig//zig:defs.bzl` | `zig_binary`, `zig_library`, `zig_test` | `@rules_dx//zig/rules:defs.bzl` | `zig_binary`, `zig_library`, `zig_test` |

Packaging and deploy:

- `@rules_dx_android//packaging:defs.bzl`: APKs, App Bundles and adb.
- `@rules_dx//appimage:defs.bzl`: AppImage bundles.
- `@rules_dx//apple/ios:device.bzl`: signed iOS IPA export.
- `@rules_dx//apple/provisioning:defs.bzl`: Apple provisioning.
- `@rules_dx//apple/provisioning:probe.bzl`: Apple toolchain probe.
- `@rules_dx//apple/signing:defs.bzl`: macOS app signing and notarization.
- `@rules_dx//deploy/offline:offline.bzl`: offline bundles.
- `@rules_dx//deploy/rules:archive.bzl`: release archives.
- `@rules_dx//deploy/rules:cloudflare.bzl`: Cloudflare deploys.
- `@rules_dx//deploy/rules:crates.bzl`: crates.io publish.
- `@rules_dx//deploy/rules:github.bzl`: GitHub releases.
- `@rules_dx//deploy/rules:maven.bzl`: Maven publish.
- `@rules_dx//deploy/rules:npm.bzl`: npm publish.
- `@rules_dx//deploy/rules:nuget.bzl`: NuGet publish.
- `@rules_dx//deploy/rules:oci.bzl`: OCI images.
- `@rules_dx//deploy/rules:octopus.bzl`: Octopus Deploy.
- `@rules_dx//deploy/rules:promotion.bzl`: release promotion.
- `@rules_dx//deploy/rules:pypi.bzl`: PyPI publish.
- `@rules_dx//msix:defs.bzl`: MSIX packages.

SDK and toolchain extensions:

- `@rules_dx_android//sdk:repos.bzl`: Android SDK.
- `@rules_dx_android//toolchain:repos.bzl`: Android NDK toolchain.
- `@rules_dx//apple/ios:simulator.bzl`: iOS simulator.
- `@rules_dx//cuda/toolchain:repos.bzl`: CUDA license acceptance.
- `@rules_dx//elixir/rules:hex.bzl`: Hex packages for Elixir.
- `@rules_dx//powershell/rules:gallery.bzl`: PowerShell Gallery modules.
- `@rules_dx//msix:repos.bzl`: MSIX tools.
- `@rules_dx//qt/sdk:repos.bzl`: Qt SDK.

API docs:

- `@rules_dx//tools/docs/go:defs.bzl`: Go API doc shards.
- `@rules_dx//tools/docs/ruby:defs.bzl`: Ruby API doc shards.
- `@rules_dx//tools/docs/jvm:defs.bzl`: Java and Kotlin API doc shards.
- `@rules_dx//tools/docs/powershell:defs.bzl`: PowerShell API doc shards.
- `@rules_dx//tools/docs/rust:defs.bzl`: Rust API doc shards.
- `@rules_dx//tools/docs/site:site.bzl`: docs sites built from API doc shards.
- `@rules_dx//tools/docs/typescript:defs.bzl`: TypeScript API doc shards.

Quality config:

- `@rules_dx//quality:native_config.bzl`: native linter and formatter configs.
- `@rules_dx//quality:policy.bzl`: quality families and workspace policy.

Every other file stays private.
A private file is not a dependency.
Readers tolerate added fields and fail on a new major schema.
The Bazel version in `.bazelversion` is the tested baseline.
Bazel 9.2.0 or later is required.

rules_dx does not provide debugger acquisition, launch or attach integration,
Debug Adapter Protocol adapters, or generated debugger configurations. These
are unsupported and not planned. Debug build profiles and editor
language-server settings do not imply debugger support. You can set up
external debuggers yourself.
