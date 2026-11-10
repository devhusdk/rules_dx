# Command Reference

Put `dx` flags after the command, Bazel flags after `--`.

```sh
tools/dx build //...
tools/dx lint //... -- --jobs=4
```

- [`dx build`](build-test-coverage.md): build targets.
- [`dx test`](build-test-coverage.md): run tests.
- [`dx coverage`](build-test-coverage.md): collect LCOV coverage.
- [`dx bench`](bench.md): run `dx-bench` tests and print their numbers.
- [`dx run`](build-test-coverage.md): build and run a target.
- [`dx deploy`](build-test-coverage.md): build and run a deployable target.
- [`dx lint`](quality.md): run linters.
- [`dx typecheck`](quality.md): run type checkers.
- [`dx format`](quality.md): check or rewrite formatting.
- [`dx check`](check.md): run generate, format, lint and typecheck.
- [`dx clean`](clean.md): reset the rebuildable state in `.dx/local`.
- [`dx generate`](generate.md): update BUILD files with Gazelle.
- [`dx sync`](sync.md): select the pinned tools and generated sources and refresh the IDE files.
- [`dx security`](audit-update.md): run the security audit.
- [`dx license`](audit-update.md): run the license audit.
- [`dx update`](audit-update.md): update dependencies.
- [`dx docs`](docs.md): build and serve the docs site.
- [`dx fuzz`](fuzz.md): run fuzz targets for a time budget.
- [`dx init`](init.md): set up a workspace.
- [`dx status`](status.md): report workspace status.
- [`dx completion`](completion.md): print shell completions.
- [`dx upgrade`](upgrade.md): move the dx pin.
- [`dx watch`](watch.md): rebuild on changes.
- [`dx worktree`](worktree.md): lease warm checkout slots for agents.
- [`dx config`](config.md#dx-config): list each tool's root config file, its base and override state.

Scope rules: [Scope Defaults](scope-defaults.md). Airgapped hosts:
[Offline Bundle](offline-bundle.md).

## Global Flags

Every command takes these.

| Flag | What it does |
| --- | --- |
| `-h`, `--help` | Print help. |
| `-V`, `--version` | Print the dx version. |
| `--workspace <dir>` | Run in another workspace. |
| `-q`, `--quiet` | Less output. |
| `-v`, `--verbose` | More output. |
| `--color auto\|always\|never` | Color output. Default `auto`. |
| `--build-slots auto\|<n>\|off` | Build slots for this run. Default `auto`. |
| `--build-slots-wait <duration>` | Give up waiting for a build slot after this long. `0` fails at once. |
| `--bazel-startup-option <token>` | Bazel startup option. Repeatable. Takes `--output_base=<path>` or `--output_user_root=<path>`. |

Other flags belong to one command family. `dx <command> --help` lists them.

| Family | Flags |
| --- | --- |
| Read commands | `-o`, `--output text\|json`. Commands that can print a patch also take `diff`. |
| Check commands | `--fail-on info\|warning\|error` (default `warning`), `--report <format>=<dest>` (repeatable). |
| Coverage | `--min-coverage <percent>`, `--min-changed-line-coverage <percent>`. |
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
[`cli/output/schema/events.schema.json`](https://github.com/devhusdk/rules_dx/blob/main/cli/output/schema/events.schema.json).
Readers accept added fields and any minor, and reject a `major` they do not write.
Every event carries `schema` and `event`. `command_finished` carries `exit_code` and `status`: `ok` for exit code 0, `failed` otherwise. Commands that run Bazel write events while they run; `dx <command> --help` marks them.

```sh
tools/dx lint //... --output=json
```

When `CLAUDECODE`, `CODEX_THREAD_ID`, `CURSOR_AGENT`, `GEMINI_CLI`, `AI_AGENT`,
`AGENT`, `GOOSE_TERMINAL`, `OPENCODE` or `AMP_CURRENT_THREAD_ID` is set, the default output is `json`. See
[Agent sessions](worktree.md#agent-sessions).

## Exit Codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | Failed check or operational failure. |
| `2` | Usage error. |
| `3` | Infrastructure failure. |
| Bazel's code | Bazel failed. |
| `101` | dx crashed. |
| `128` plus signal | A signal stopped dx. |

## Build Slots

Commands that run Bazel wait for a build slot, so parallel runs on one machine
do not run out of memory. Slots are shared by every worktree on the machine.
The count is the machine memory, or the cgroup limit if lower, divided by 3 GiB.
It is at least 1 and at most 16. `DX_BUILD_SLOT_MEMORY` replaces the 3 GiB:
MiB, or a number ending in `m` or `g`. An unreadable or zero value is ignored.

```sh
tools/dx build //... --build-slots 4
```

- `auto`: the memory-based count. This is the default inside a
  `dx worktree` slot.
- `<n>`: exactly `n` slots, from 1 to 1024.
- `off`: take no slot. This is the default outside a `dx worktree` slot.

`DX_BUILD_SLOTS` sets a default. The flag wins. A waiting run prints
`dx: waiting for a build slot` to stderr and runs when a slot frees. Run
`dx status` to see who holds the slots. Ctrl-C ends the wait.

By default a run waits for ever. `--build-slots-wait <duration>` or
`DX_BUILD_SLOTS_WAIT` sets a limit: seconds, or a number ending in `s`, `m` or
`h`. `0` fails at once when no slot is free. When the limit passes, dx exits 1
with the `slots_busy` error, which names the holders, and in JSON mode ends the
stream with an `error` event. A free slot is always taken, whatever the limit.

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
tools/dx worktree prune --apply
```

## Environment

A flag on the command line wins over these defaults.

| Variable | What it does |
| --- | --- |
| `DX_OUTPUT=<mode>` | Default for `--output`. |
| `DX_BUILD_SLOTS=auto\|<n>\|off` | Default for `--build-slots`. |
| `DX_BUILD_SLOTS_WAIT=<duration>` | Default for `--build-slots-wait`. |
| `DX_BUILD_SLOT_MEMORY=<size>` | Memory planned per build slot, default `3g`. |
| `NO_COLOR=<non-empty>` | Disable color in `auto` mode. |
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

## Main Entry Points

Binary and test wrappers pick the program they run the same way. `main` is
the source named `<name>.<ext>`, else the single entry in `srcs`, else the
rule fails and asks for `main`. `main` is always linted unless it is an
external label.

## Supported Surface

`@rules_dx//:dx` runs every command.

Language rules:

- `@rules_dx//android:defs.bzl`: Android libraries.
- `@rules_dx//api_spec:defs.bzl`: OpenAPI and AsyncAPI specs.
- `@rules_dx//apple:defs.bzl`: iOS apps, tests and frameworks.
- `@rules_dx//apple:defs.bzl`: iOS libraries.
- `@rules_dx//apple:defs.bzl`: macOS apps.
- `@rules_dx//cc:defs.bzl`: C and C++.
- `@rules_dx//csharp:defs.bzl`: C#.
- `@rules_dx//cuda:defs.bzl`: CUDA.
- `@rules_dx//dockerfile:defs.bzl`: Dockerfiles.
- `@rules_dx//make:defs.bzl`: Makefiles.
- `@rules_dx//cmake:defs.bzl`: CMake files.
- `@rules_dx//d:defs.bzl`: D.
- `@rules_dx//elixir:defs.bzl`: Elixir.
- `@rules_dx//erlang:defs.bzl`: Erlang.
- `@rules_dx//fsharp:defs.bzl`: F#.
- `@rules_dx//go:defs.bzl`: Go.
- `@rules_dx//haskell:defs.bzl`: Haskell.
- `@rules_dx//java:defs.bzl`: Java.
- `@rules_dx//javascript:defs.bzl`: JavaScript.
- `@rules_dx//ansible:defs.bzl`: Ansible.
- `@rules_dx//bicep:defs.bzl`: Bicep.
- `@rules_dx//cloudformation:defs.bzl`: CloudFormation.
- `@rules_dx//julia:defs.bzl`: Julia.
- `@rules_dx//kotlin:defs.bzl`: Kotlin.
- `@rules_dx//lua:defs.bzl`: Lua.
- `@rules_dx//kubernetes:defs.bzl`: Kubernetes.
- `@rules_dx//perl:defs.bzl`: Perl.
- `@rules_dx//php:defs.bzl`: PHP.
- `@rules_dx//powershell:defs.bzl`: PowerShell.
- `@rules_dx//python:defs.bzl`: Python.
- `@rules_dx//qt:defs.bzl`: Qt.
- `@rules_dx//core/quality:defs.bzl`: CUE, Gherkin, HTML templates, Jsonnet, Jupyter, LaTeX, Nix, Pkl, Typst, XML, YAML.
- `@rules_dx//rego:defs.bzl`: Rego.
- `@rules_dx//ruby:defs.bzl`: Ruby.
- `@rules_dx//rust:defs.bzl`: Rust.
- `@rules_dx//scala:defs.bzl`: Scala.
- `@rules_dx//sql:defs.bzl`: SQL.
- `@rules_dx//terraform:defs.bzl`: Terraform.
- `@rules_dx//typescript:defs.bzl`: TypeScript.
- `@rules_dx//vue:defs.bzl`: Vue.
- `@rules_dx//zig:defs.bzl`: Zig.

### Rule names

dx names rules `<language>_<kind>` throughout, so `py_library` is `python_library`, `ts_project` is `typescript_library` and `kt_jvm_library` is `kotlin_library`. This table maps each upstream rule to the
dx rule that wraps it.

| Upstream load | Upstream rule | dx load | dx rule |
| --- | --- | --- | --- |
| `@rules_android//rules:rules.bzl` | `android_library` | `@rules_dx//android:defs.bzl` | `java_android_library` |
| `@rules_kotlin//kotlin:android.bzl` | `kt_android_library` | `@rules_dx//android:defs.bzl` | `kotlin_android_library` |
| `@rules_apple//apple:ios.bzl` | `ios_application` | `@rules_dx//apple:defs.bzl` | `apple_ios_app` |
| `@rules_apple//apple:ios.bzl` | `ios_unit_test` | `@rules_dx//apple:defs.bzl` | `apple_ios_app_test` |
| `@rules_apple//apple:apple.bzl` | `apple_xcframework` | `@rules_dx//apple:defs.bzl` | `apple_ios_xcframework` |
| `@rules_apple//apple:macos.bzl` | `macos_application` | `@rules_dx//apple:defs.bzl` | `apple_macos_app` |
| `@rules_cc//cc:defs.bzl` | `cc_binary`, `cc_library`, `cc_test` | `@rules_dx//cc:defs.bzl` | `cc_binary`, `cc_library`, `cc_test` |
| `@rules_cc//cc:objc_library.bzl` | `objc_library` | `@rules_dx//cc:defs.bzl` | `objc_library` |
| `@rules_dotnet//dotnet:defs.bzl` | `csharp_binary`, `csharp_library`, `csharp_test` | `@rules_dx//csharp:defs.bzl` | `csharp_binary`, `csharp_library`, `csharp_test` |
| `@rules_cuda//cuda:defs.bzl` | `cuda_library`, `cuda_objects` | `@rules_dx//cuda:defs.bzl` | `cuda_library`, `cuda_objects` |
| `@rules_d//d:defs.bzl` | `d_binary`, `d_library`, `d_test` | `@rules_dx//d:defs.bzl` | `d_binary`, `d_library`, `d_test` |
| `@rules_elixir//:elixir_app.bzl` | `elixir_app` | `@rules_dx//elixir:defs.bzl` | `elixir_app` |
| `@rules_elixir//:ex_unit_test.bzl` | `ex_unit_test` | `@rules_dx//elixir:defs.bzl` | `elixir_test` |
| `@rules_erlang//:erlang_app.bzl` | `erlang_app`, `test_erlang_app` | `@rules_dx//erlang:defs.bzl` | `erlang_app`, `erlang_test_app` |
| `@rules_erlang//:ct.bzl` | `ct_suite` | `@rules_dx//erlang:defs.bzl` | `erlang_ct_suite_test` |
| `@rules_erlang//:erlang_bytecode2.bzl` | `erlang_bytecode` | `@rules_dx//erlang:defs.bzl` | `erlang_bytecode` |
| `@rules_erlang//:escript.bzl` | `escript_archive` | `@rules_dx//erlang:defs.bzl` | `erlang_escript_archive` |
| `@rules_erlang//:eunit2.bzl` | `eunit` | `@rules_dx//erlang:defs.bzl` | `erlang_eunit_test` |
| `@rules_dotnet//dotnet:defs.bzl` | `fsharp_binary`, `fsharp_library`, `fsharp_test` | `@rules_dx//fsharp:defs.bzl` | `fsharp_binary`, `fsharp_library`, `fsharp_test` |
| `@rules_go//go:def.bzl` | `go_binary`, `go_library`, `go_test` | `@rules_dx//go:defs.bzl` | `go_binary`, `go_library`, `go_test` |
| `@rules_haskell//haskell:defs.bzl` | `haskell_binary`, `haskell_library`, `haskell_test` | `@rules_dx//haskell:defs.bzl` | `haskell_binary`, `haskell_library`, `haskell_test` |
| `@rules_java//java:defs.bzl` | `java_binary`, `java_library`, `java_test` | `@rules_dx//java:defs.bzl` | `java_binary`, `java_library`, `java_test` |
| `@aspect_rules_js//js:defs.bzl` | `js_binary` | `@rules_dx//javascript:defs.bzl` | `javascript_binary` |
| `@aspect_rules_js//js:defs.bzl` | `js_library` | `@rules_dx//javascript:defs.bzl` | `javascript_library` |
| `@aspect_rules_js//js:defs.bzl` | `js_test` | `@rules_dx//javascript:defs.bzl` | `javascript_test` |
| `@aspect_rules_jest//jest:defs.bzl` | `jest_test` | `@rules_dx//javascript:defs.bzl` | `javascript_test` |
| `@rules_julia//julia:defs.bzl` | `julia_binary`, `julia_library`, `julia_test` | `@rules_dx//julia:defs.bzl` | `julia_binary`, `julia_library`, `julia_test` |
| `@rules_kotlin//kotlin:jvm.bzl` | `kt_jvm_binary`, `kt_jvm_library`, `kt_jvm_test` | `@rules_dx//kotlin:defs.bzl` | `kotlin_binary`, `kotlin_library`, `kotlin_test` |
| `@rules_perl//perl:defs.bzl` | `perl_binary`, `perl_library`, `perl_test` | `@rules_dx//perl:defs.bzl` | `perl_binary`, `perl_library`, `perl_test` |
| `@rules_powershell//powershell:defs.bzl` | `pwsh_binary`, `pwsh_library`, `pwsh_test` | `@rules_dx//powershell:defs.bzl` | `powershell_binary`, `powershell_library`, `powershell_test` |
| `@rules_py//py:defs.bzl` | `py_binary` | `@rules_dx//python:defs.bzl` | `python_binary` |
| `@rules_py//py:defs.bzl` | `py_library` | `@rules_dx//python:defs.bzl` | `python_library` |
| `@rules_py//py:defs.bzl` | `py_test`, `py_pytest_test` | `@rules_dx//python:defs.bzl` | `python_test` |
| `@rules_ruby//ruby:defs.bzl` | `rb_binary`, `rb_library`, `rb_test` | `@rules_dx//ruby:defs.bzl` | `ruby_binary`, `ruby_library`, `ruby_test` |
| `@rules_rust//rust:defs.bzl` | `rust_binary`, `rust_library`, `rust_proc_macro`, `rust_shared_library`, `rust_static_library`, `rust_test` | `@rules_dx//rust:defs.bzl` | same names |
| `@rules_scala//scala:scala.bzl` | `scala_binary`, `scala_library`, `scala_test` | `@rules_dx//scala:defs.bzl` | `scala_binary`, `scala_library`, `scala_test` |
| `@aspect_rules_js//js:defs.bzl` | `js_binary` | `@rules_dx//typescript:defs.bzl` | `typescript_binary` |
| `@aspect_rules_ts//ts:defs.bzl` | `ts_project` | `@rules_dx//typescript:defs.bzl` | `typescript_library` |
| `@aspect_rules_js//js:defs.bzl` | `js_test` | `@rules_dx//typescript:defs.bzl` | `typescript_test` |
| `@aspect_rules_jest//jest:defs.bzl` | `jest_test` | `@rules_dx//typescript:defs.bzl` | `typescript_test` |
| `@aspect_rules_js//js:defs.bzl` | `js_library` | `@rules_dx//vue:defs.bzl` | `vue_library` |
| `@rules_zig//zig:defs.bzl` | `zig_binary`, `zig_library`, `zig_test` | `@rules_dx//zig:defs.bzl` | `zig_binary`, `zig_library`, `zig_test` |
|  |  | `@rules_dx//android:defs.bzl` | `android_adb_install` |
|  |  | `@rules_dx//android:defs.bzl` | `android_adb_log` |
|  |  | `@rules_dx//android:defs.bzl` | `android_adb_start` |
|  |  | `@rules_dx//android:defs.bzl` | `android_apk_set` |
|  |  | `@rules_dx//android:defs.bzl` | `android_check` |
|  |  | `@rules_dx//android:defs.bzl` | `android_native_library` |
|  |  | `@rules_dx//android:defs.bzl` | `android_sign` |
|  |  | `@rules_dx//android:defs.bzl` | `rust_android_apk` |
|  |  | `@rules_dx//ansible:defs.bzl` | `ansible_library` |
|  |  | `@rules_dx//api_spec:defs.bzl` | `api_spec_library` |
|  |  | `@rules_dx//appimage:defs.bzl` | `AppImageInfo` |
|  |  | `@rules_dx//appimage:defs.bzl` | `appimage_package` |
|  |  | `@rules_dx//apple:defs.bzl` | `apple_ios_export` |
|  |  | `@rules_dx//apple:defs.bzl` | `apple_ios_lib` |
|  |  | `@rules_dx//apple:defs.bzl` | `apple_macos_notarize` |
|  |  | `@rules_dx//apple:defs.bzl` | `apple_macos_sign` |
|  |  | `@rules_dx//apple:defs.bzl` | `apple_provisioning` |
|  |  | `@rules_dx//apple:defs.bzl` | `apple_simulator_boot` |
|  |  | `@rules_dx//apple:defs.bzl` | `apple_simulator_install` |
|  |  | `@rules_dx//apple:defs.bzl` | `apple_simulator_launch` |
|  |  | `@rules_dx//apple:defs.bzl` | `apple_toolchain_probe` |
|  |  | `@rules_dx//asciidoc:defs.bzl` | `asciidoc_library` |
|  |  | `@rules_dx//astro:defs.bzl` | `astro_build` |
|  |  | `@rules_dx//astro:defs.bzl` | `astro_library` |
|  |  | `@rules_dx//astro:defs.bzl` | `astro_test` |
|  |  | `@rules_dx//avro:defs.bzl` | `avro_library` |
|  |  | `@rules_dx//bicep:defs.bzl` | `bicep_library` |
|  |  | `@rules_dx//bicep:defs.bzl` | `bicep_registry_module` |
|  |  | `@rules_dx//capnproto:defs.bzl` | `capnp_library` |
|  |  | `@rules_dx//capnproto:defs.bzl` | `std_import_path` |
|  |  | `@rules_dx//cc:defs.bzl` | `cc_bench` |
|  |  | `@rules_dx//cc:defs.bzl` | `cc_coverage_features` |
|  |  | `@rules_dx//cc:defs.bzl` | `cc_coverage_mismatch` |
|  |  | `@rules_dx//cc:defs.bzl` | `cc_coverage_probe` |
|  |  | `@rules_dx//cc:defs.bzl` | `cc_coverage_settings` |
|  |  | `@rules_dx//cc:defs.bzl` | `cc_coverage_unsupported` |
|  |  | `@rules_dx//cc:defs.bzl` | `cc_fuzz_test` |
|  |  | `@rules_dx//cc:defs.bzl` | `cc_param_file_features` |
|  |  | `@rules_dx//cloudformation:defs.bzl` | `cloudformation_library` |
|  |  | `@rules_dx//cmake:defs.bzl` | `cmake_library` |
|  |  | `@rules_dx//csharp:defs.bzl` | `csharp_bench` |
|  |  | `@rules_dx//csharp:defs.bzl` | `csharp_fuzz_test` |
|  |  | `@rules_dx//csharp:defs.bzl` | `csharp_proto_library` |
|  |  | `@rules_dx//cuda:defs.bzl` | `cuda_binary` |
|  |  | `@rules_dx//cuda:defs.bzl` | `cuda_component_library` |
|  |  | `@rules_dx//cuda:defs.bzl` | `cuda_gpu_env` |
|  |  | `@rules_dx//cuda:defs.bzl` | `cuda_test` |
|  |  | `@rules_dx//d:defs.bzl` | `d_bench` |
|  |  | `@rules_dx//d:defs.bzl` | `d_fuzz_test` |
|  |  | `@rules_dx//d:defs.bzl` | `d_quality_srcs` |
|  |  | `@rules_dx//dart:defs.bzl` | `dart_bench` |
|  |  | `@rules_dx//dart:defs.bzl` | `dart_binary` |
|  |  | `@rules_dx//dart:defs.bzl` | `dart_codegen` |
|  |  | `@rules_dx//dart:defs.bzl` | `dart_js_binary` |
|  |  | `@rules_dx//dart:defs.bzl` | `dart_library` |
|  |  | `@rules_dx//dart:defs.bzl` | `dart_root_package_name` |
|  |  | `@rules_dx//dart:defs.bzl` | `dart_test` |
|  |  | `@rules_dx//dart:defs.bzl` | `dart_wasm_binary` |
|  |  | `@rules_dx//dockerfile:defs.bzl` | `dockerfile_library` |
|  |  | `@rules_dx//elixir:defs.bzl` | `elixir_bench` |
|  |  | `@rules_dx//elixir:defs.bzl` | `escript_archive` |
|  |  | `@rules_dx//elixir:defs.bzl` | `escript_headers` |
|  |  | `@rules_dx//env:defs.bzl` | `EnvironmentInfo` |
|  |  | `@rules_dx//env:defs.bzl` | `env_host_filename` |
|  |  | `@rules_dx//env:defs.bzl` | `env_tool_record` |
|  |  | `@rules_dx//env:defs.bzl` | `env_tree_metadata` |
|  |  | `@rules_dx//env:defs.bzl` | `environment_config` |
|  |  | `@rules_dx//env:defs.bzl` | `environment_metadata` |
|  |  | `@rules_dx//env:defs.bzl` | `environment_tool` |
|  |  | `@rules_dx//env:defs.bzl` | `environment_tree` |
|  |  | `@rules_dx//erlang:defs.bzl` | `erlang_bench` |
|  |  | `@rules_dx//erlang:defs.bzl` | `erlang_nif` |
|  |  | `@rules_dx//erlang:defs.bzl` | `erlang_release` |
|  |  | `@rules_dx//erlang:defs.bzl` | `erlang_xrl_yrl` |
|  |  | `@rules_dx//erlang:defs.bzl` | `erlc_opts` |
|  |  | `@rules_dx//flatbuffers:defs.bzl` | `flatbuffers_library` |
|  |  | `@rules_dx//flatbuffers:defs.bzl` | `resolve_root` |
|  |  | `@rules_dx//flutter:defs.bzl` | `flutter_android_app` |
|  |  | `@rules_dx//flutter:defs.bzl` | `flutter_application` |
|  |  | `@rules_dx//flutter:defs.bzl` | `flutter_gen_l10n` |
|  |  | `@rules_dx//flutter:defs.bzl` | `flutter_ios_app` |
|  |  | `@rules_dx//flutter:defs.bzl` | `flutter_library` |
|  |  | `@rules_dx//flutter:defs.bzl` | `flutter_test` |
|  |  | `@rules_dx//flutter:defs.bzl` | `flutter_web_app` |
|  |  | `@rules_dx//flutter:defs.bzl` | `flutter_web_bundle` |
|  |  | `@rules_dx//fortran:defs.bzl` | `fortran_binary` |
|  |  | `@rules_dx//fortran:defs.bzl` | `fortran_library` |
|  |  | `@rules_dx//fortran:defs.bzl` | `fortran_test` |
|  |  | `@rules_dx//fortran:defs.bzl` | `library_name` |
|  |  | `@rules_dx//fsharp:defs.bzl` | `fsharp_bench` |
|  |  | `@rules_dx//fsharp:defs.bzl` | `fsharp_fuzz_test` |
|  |  | `@rules_dx//go:defs.bzl` | `go_bench` |
|  |  | `@rules_dx//go:defs.bzl` | `go_fuzz_test` |
|  |  | `@rules_dx//haskell:defs.bzl` | `haskell_bench` |
|  |  | `@rules_dx//helm:defs.bzl` | `helm_chart` |
|  |  | `@rules_dx//java:defs.bzl` | `java_bench` |
|  |  | `@rules_dx//java:defs.bzl` | `java_fuzz_test` |
|  |  | `@rules_dx//java:defs.bzl` | `java_jmh_test` |
|  |  | `@rules_dx//javascript:defs.bzl` | `javascript_bench` |
|  |  | `@rules_dx//javascript:defs.bzl` | `javascript_binary_upstream_data` |
|  |  | `@rules_dx//javascript:defs.bzl` | `javascript_fuzz_test` |
|  |  | `@rules_dx//javascript:defs.bzl` | `javascript_test_rejection` |
|  |  | `@rules_dx//javascript:defs.bzl` | `javascript_wasm_bindgen_library` |
|  |  | `@rules_dx//julia:defs.bzl` | `julia_standalone_binary` |
|  |  | `@rules_dx//kotlin:defs.bzl` | `kotlin_bench` |
|  |  | `@rules_dx//kotlin:defs.bzl` | `kotlin_fuzz_test` |
|  |  | `@rules_dx//kotlin:defs.bzl` | `kotlin_warnings_as_errors_opts` |
|  |  | `@rules_dx//kubernetes:defs.bzl` | `kubernetes_library` |
|  |  | `@rules_dx//kubernetes:defs.bzl` | `kubernetes_schemas` |
|  |  | `@rules_dx//lua:defs.bzl` | `lua_binary` |
|  |  | `@rules_dx//lua:defs.bzl` | `lua_library` |
|  |  | `@rules_dx//lua:defs.bzl` | `lua_test` |
|  |  | `@rules_dx//make:defs.bzl` | `make_library` |
|  |  | `@rules_dx//mdx:defs.bzl` | `mdx_build` |
|  |  | `@rules_dx//mdx:defs.bzl` | `mdx_library` |
|  |  | `@rules_dx//mdx:defs.bzl` | `mdx_test` |
|  |  | `@rules_dx//msix:defs.bzl` | `msix_package` |
|  |  | `@rules_dx//msix:defs.bzl` | `msix_sign` |
|  |  | `@rules_dx//ocaml:defs.bzl` | `dx_dune_library` |
|  |  | `@rules_dx//ocaml:defs.bzl` | `ocaml_bench` |
|  |  | `@rules_dx//ocaml:defs.bzl` | `ocaml_binary` |
|  |  | `@rules_dx//ocaml:defs.bzl` | `ocaml_fuzz_test` |
|  |  | `@rules_dx//ocaml:defs.bzl` | `ocaml_library` |
|  |  | `@rules_dx//ocaml:defs.bzl` | `ocaml_module` |
|  |  | `@rules_dx//ocaml:defs.bzl` | `ocaml_signature` |
|  |  | `@rules_dx//ocaml:defs.bzl` | `ocaml_test` |
|  |  | `@rules_dx//ocaml:defs.bzl` | `ocamllex` |
|  |  | `@rules_dx//perl:defs.bzl` | `perl_bench` |
|  |  | `@rules_dx//php:defs.bzl` | `php_bench` |
|  |  | `@rules_dx//php:defs.bzl` | `php_binary` |
|  |  | `@rules_dx//php:defs.bzl` | `php_library` |
|  |  | `@rules_dx//php:defs.bzl` | `php_test` |
|  |  | `@rules_dx//python:defs.bzl` | `python_bench` |
|  |  | `@rules_dx//python:defs.bzl` | `python_fuzz_test` |
|  |  | `@rules_dx//python:defs.bzl` | `python_grpc_library` |
|  |  | `@rules_dx//python:defs.bzl` | `python_main` |
|  |  | `@rules_dx//python:defs.bzl` | `python_main_rejection` |
|  |  | `@rules_dx//python:defs.bzl` | `python_test_rejection` |
|  |  | `@rules_dx//qt:defs.bzl` | `qt_bundle` |
|  |  | `@rules_dx//qt:defs.bzl` | `qt_bundle_members` |
|  |  | `@rules_dx//qt:defs.bzl` | `qt_cc_binary` |
|  |  | `@rules_dx//qt:defs.bzl` | `qt_cc_library` |
|  |  | `@rules_dx//qt:defs.bzl` | `qt_cc_test` |
|  |  | `@rules_dx//qt:defs.bzl` | `qt_consumer_identity` |
|  |  | `@rules_dx//qt:defs.bzl` | `qt_framework_path` |
|  |  | `@rules_dx//qt:defs.bzl` | `qt_frameworks` |
|  |  | `@rules_dx//qt:defs.bzl` | `qt_identifier` |
|  |  | `@rules_dx//qt:defs.bzl` | `qt_library_stem` |
|  |  | `@rules_dx//qt:defs.bzl` | `qt_module_path` |
|  |  | `@rules_dx//qt:defs.bzl` | `qt_os_supplied_libraries` |
|  |  | `@rules_dx//qt:defs.bzl` | `qt_package_path` |
|  |  | `@rules_dx//qt:defs.bzl` | `qt_qml_module` |
|  |  | `@rules_dx//qt:defs.bzl` | `qt_qmldir` |
|  |  | `@rules_dx//qt:defs.bzl` | `qt_qrc` |
|  |  | `@rules_dx//qt:defs.bzl` | `qt_runtime_members` |
|  |  | `@rules_dx//qt:defs.bzl` | `qt_rust_cc_library` |
|  |  | `@rules_dx//qt:defs.bzl` | `qt_rust_runtime` |
|  |  | `@rules_dx//qt:defs.bzl` | `qt_version_parts` |
|  |  | `@rules_dx//rego:defs.bzl` | `rego_library` |
|  |  | `@rules_dx//rego:defs.bzl` | `rego_test` |
|  |  | `@rules_dx//rst:defs.bzl` | `rst_library` |
|  |  | `@rules_dx//ruby:defs.bzl` | `ruby_bench` |
|  |  | `@rules_dx//ruby:defs.bzl` | `ruby_coverage_lib_dirs` |
|  |  | `@rules_dx//rust:defs.bzl` | `rust_bench` |
|  |  | `@rules_dx//rust:defs.bzl` | `rust_binary` |
|  |  | `@rules_dx//rust:defs.bzl` | `rust_fuzz_test` |
|  |  | `@rules_dx//rust:defs.bzl` | `rust_library` |
|  |  | `@rules_dx//rust:defs.bzl` | `rust_proc_macro` |
|  |  | `@rules_dx//rust:defs.bzl` | `rust_shared_library` |
|  |  | `@rules_dx//rust:defs.bzl` | `rust_static_library` |
|  |  | `@rules_dx//rust:defs.bzl` | `rust_test` |
|  |  | `@rules_dx//scala:defs.bzl` | `scala_bench` |
|  |  | `@rules_dx//scala:defs.bzl` | `scala_fuzz_test` |
|  |  | `@rules_dx//sql:defs.bzl` | `sql_library` |
|  |  | `@rules_dx//svelte:defs.bzl` | `svelte_build` |
|  |  | `@rules_dx//svelte:defs.bzl` | `svelte_library` |
|  |  | `@rules_dx//svelte:defs.bzl` | `svelte_test` |
|  |  | `@rules_dx//swift:defs.bzl` | `swift_bench` |
|  |  | `@rules_dx//swift:defs.bzl` | `swift_binary` |
|  |  | `@rules_dx//swift:defs.bzl` | `swift_fuzz_test` |
|  |  | `@rules_dx//swift:defs.bzl` | `swift_interop_hint` |
|  |  | `@rules_dx//swift:defs.bzl` | `swift_library` |
|  |  | `@rules_dx//swift:defs.bzl` | `swift_test` |
|  |  | `@rules_dx//terraform:defs.bzl` | `collect_providers` |
|  |  | `@rules_dx//terraform:defs.bzl` | `terraform_library` |
|  |  | `@rules_dx//thrift:defs.bzl` | `thrift_library` |
|  |  | `@rules_dx//typescript:defs.bzl` | `dx_scoped_tsconfig_out` |
|  |  | `@rules_dx//typescript:defs.bzl` | `dx_tsconfig_content` |
|  |  | `@rules_dx//typescript:defs.bzl` | `dx_tsconfig_is_source` |
|  |  | `@rules_dx//typescript:defs.bzl` | `dx_tsconfig_relpath` |
|  |  | `@rules_dx//typescript:defs.bzl` | `typescript_bench` |
|  |  | `@rules_dx//typescript:defs.bzl` | `typescript_binary_forward_kwargs` |
|  |  | `@rules_dx//typescript:defs.bzl` | `typescript_binary_kwargs_rejection` |
|  |  | `@rules_dx//typescript:defs.bzl` | `typescript_binary_main` |
|  |  | `@rules_dx//typescript:defs.bzl` | `typescript_binary_rejection` |
|  |  | `@rules_dx//typescript:defs.bzl` | `typescript_binary_upstream_kwargs` |
|  |  | `@rules_dx//typescript:defs.bzl` | `typescript_compiled_entry` |
|  |  | `@rules_dx//typescript:defs.bzl` | `typescript_declaration_srcs_rejection` |
|  |  | `@rules_dx//typescript:defs.bzl` | `typescript_empty_library_rejection` |
|  |  | `@rules_dx//typescript:defs.bzl` | `typescript_fuzz_test` |
|  |  | `@rules_dx//typescript:defs.bzl` | `typescript_playwright_test` |
|  |  | `@rules_dx//typescript:defs.bzl` | `typescript_proto_library` |
|  |  | `@rules_dx//typescript:defs.bzl` | `typescript_scoped_tsconfig_rejection` |
|  |  | `@rules_dx//typescript:defs.bzl` | `typescript_srcs_rejection` |
|  |  | `@rules_dx//typescript:defs.bzl` | `typescript_test_rejection` |
|  |  | `@rules_dx//typescript:defs.bzl` | `typescript_transpiler` |
|  |  | `@rules_dx//typescript:defs.bzl` | `typescript_upstream_srcs` |
|  |  | `@rules_dx//vue:defs.bzl` | `vue_build` |
|  |  | `@rules_dx//vue:defs.bzl` | `vue_test` |
|  |  | `@rules_dx//zig:defs.bzl` | `zig_bench` |
|  |  | `@rules_dx//zig:defs.bzl` | `zig_import_name` |

Packaging and deploy:

- `@rules_dx//android:defs.bzl`: APKs, App Bundles and adb.
- `@rules_dx//appimage:defs.bzl`: AppImage bundles.
- `@rules_dx//apple/private/ios:device.bzl`: signed iOS IPA export.
- `@rules_dx//apple/private/provisioning:defs.bzl`: Apple provisioning.
- `@rules_dx//apple/private/provisioning:probe.bzl`: Apple toolchain probe.
- `@rules_dx//apple/private/signing:defs.bzl`: macOS app signing and notarization.
- `@rules_dx//deploy:defs.bzl`: deploy rules, release archives and offline bundles.
- `@rules_dx//msix:defs.bzl`: MSIX packages.

SDK and toolchain extensions:

- `@rules_dx//android:extensions.bzl`: Android SDK.
- `@rules_dx//android:repos.bzl`: Android NDK toolchain.
- `@rules_dx//apple/private/ios:simulator.bzl`: iOS simulator.
- `@rules_dx//cuda:extensions.bzl`: CUDA license acceptance.
- `@rules_dx//elixir:extensions.bzl`: Hex packages for Elixir.
- `@rules_dx//powershell:extensions.bzl`: PowerShell Gallery modules.
- `@rules_dx//msix:extensions.bzl`: MSIX tools.
- `@rules_dx//qt:extensions.bzl`: Qt SDK.

API docs:

- `@rules_dx//tools/docs/dart:defs.bzl`: Dart API doc shards.
- `@rules_dx//tools/docs/elixir:defs.bzl`: Elixir API doc shards.
- `@rules_dx//tools/docs/erlang:defs.bzl`: Erlang API doc shards.
- `@rules_dx//tools/docs/cc:defs.bzl`: C, C++ and CUDA API doc shards.
- `@rules_dx//tools/docs/go:defs.bzl`: Go API doc shards.
- `@rules_dx//tools/docs/ruby:defs.bzl`: Ruby API doc shards.
- `@rules_dx//tools/docs/julia:defs.bzl`: Julia API doc shards.
- `@rules_dx//tools/docs/javascript:defs.bzl`: JavaScript API doc shards.
- `@rules_dx//tools/docs/lua:defs.bzl`: Lua API doc shards.
- `@rules_dx//tools/docs/jvm:defs.bzl`: Java and Kotlin API doc shards.
- `@rules_dx//tools/docs/perl:defs.bzl`: Perl API doc shards.
- `@rules_dx//tools/docs/php:defs.bzl`: PHP API doc shards.
- `@rules_dx//tools/docs/powershell:defs.bzl`: PowerShell API doc shards.
- `@rules_dx//tools/docs/rust:defs.bzl`: Rust API doc shards.
- `@rules_dx//tools/docs/site:site.bzl`: docs sites built from API doc shards.
- `@rules_dx//tools/docs/typescript:defs.bzl`: TypeScript API doc shards.

Quality config:

- `@rules_dx//core/quality:native_config.bzl`: native linter and formatter configs.

Every other file stays private.
A private file is not a dependency.
The Bazel version in `.bazelversion` is the tested baseline.
Bazel 9.2.0 or later is required.

rules_dx does not provide debugger acquisition, launch or attach integration,
Debug Adapter Protocol adapters, or generated debugger configurations. These
are unsupported and not planned. Debug build modes and editor
language-server settings do not imply debugger support. You can set up
external debuggers yourself.
