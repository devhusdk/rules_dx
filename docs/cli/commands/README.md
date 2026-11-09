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
| Bazel's code | Bazel failed. |
| `101` | dx crashed. |
| `128` plus signal | A signal stopped dx. |

## Build Slots

Commands that run Bazel wait for a build slot, so parallel runs on one machine
do not run out of memory. Slots are shared by every worktree on the machine.
The count is the machine memory, or the cgroup limit if lower, divided by 3 GiB.
It is at least 1 and at most 16.

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

- `@rules_dx_android//rules:defs.bzl`: Android libraries.
- `@rules_dx//api_spec/rules:defs.bzl`: OpenAPI and AsyncAPI specs.
- `@rules_dx_apple//ios:app.bzl`: iOS apps, tests and frameworks.
- `@rules_dx_apple//ios:defs.bzl`: iOS libraries.
- `@rules_dx_apple//macos:app.bzl`: macOS apps.
- `@rules_dx//cc/rules:defs.bzl`: C and C++.
- `@rules_dx_dotnet//csharp/rules:defs.bzl`: C#.
- `@rules_dx_cuda//rules:defs.bzl`: CUDA.
- `@rules_dx//dockerfile/rules:defs.bzl`: Dockerfiles.
- `@rules_dx//make/rules:defs.bzl`: Makefiles.
- `@rules_dx//cmake/rules:defs.bzl`: CMake files.
- `@rules_dx//d/rules:defs.bzl`: D.
- `@rules_dx//elixir/rules:defs.bzl`: Elixir.
- `@rules_dx//erlang/rules:defs.bzl`: Erlang.
- `@rules_dx_dotnet//fsharp/rules:defs.bzl`: F#.
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

dx names rules `<language>_<kind>` throughout, so `py_library` is `python_library`, `ts_project` is `typescript_project` and `kt_jvm_library` is `kotlin_library`. This table maps each upstream rule to the
dx rule that wraps it.

| Upstream load | Upstream rule | dx load | dx rule |
| --- | --- | --- | --- |
| `@rules_android//rules:rules.bzl` | `android_library` | `@rules_dx_android//rules:defs.bzl` | `java_android_library` |
| `@rules_kotlin//kotlin:android.bzl` | `kt_android_library` | `@rules_dx_android//rules:defs.bzl` | `kotlin_android_library` |
| `@rules_apple//apple:ios.bzl` | `ios_application` | `@rules_dx_apple//ios:app.bzl` | `apple_ios_app` |
| `@rules_apple//apple:ios.bzl` | `ios_unit_test` | `@rules_dx_apple//ios:app.bzl` | `apple_ios_app_test` |
| `@rules_apple//apple:apple.bzl` | `apple_xcframework` | `@rules_dx_apple//ios:app.bzl` | `apple_ios_xcframework` |
| `@rules_apple//apple:macos.bzl` | `macos_application` | `@rules_dx_apple//macos:app.bzl` | `apple_macos_app` |
| `@rules_cc//cc:defs.bzl` | `cc_binary`, `cc_library`, `cc_test` | `@rules_dx//cc/rules:defs.bzl` | `cc_binary`, `cc_library`, `cc_test` |
| `@rules_cc//cc:objc_library.bzl` | `objc_library` | `@rules_dx//cc/rules:defs.bzl` | `objc_library` |
| `@rules_dotnet//dotnet:defs.bzl` | `csharp_binary`, `csharp_library`, `csharp_test` | `@rules_dx_dotnet//csharp/rules:defs.bzl` | `csharp_binary`, `csharp_library`, `csharp_test` |
| `@rules_cuda//cuda:defs.bzl` | `cuda_library`, `cuda_objects` | `@rules_dx_cuda//rules:defs.bzl` | `cuda_library`, `cuda_objects` |
| `@rules_d//d:defs.bzl` | `d_binary`, `d_library`, `d_test` | `@rules_dx//d/rules:defs.bzl` | `d_binary`, `d_library`, `d_test` |
| `@rules_elixir//:elixir_app.bzl` | `elixir_app` | `@rules_dx//elixir/rules:defs.bzl` | `elixir_app` |
| `@rules_elixir//:ex_unit_test.bzl` | `ex_unit_test` | `@rules_dx//elixir/rules:defs.bzl` | `elixir_test` |
| `@rules_erlang//:erlang_app.bzl` | `erlang_app`, `test_erlang_app` | `@rules_dx//erlang/rules:defs.bzl` | `erlang_app`, `erlang_test_app` |
| `@rules_erlang//:ct.bzl` | `ct_suite` | `@rules_dx//erlang/rules:defs.bzl` | `erlang_ct_suite_test` |
| `@rules_erlang//:erlang_bytecode2.bzl` | `erlang_bytecode` | `@rules_dx//erlang/rules:defs.bzl` | `erlang_bytecode` |
| `@rules_erlang//:escript.bzl` | `escript_archive` | `@rules_dx//erlang/rules:defs.bzl` | `erlang_escript_archive` |
| `@rules_erlang//:eunit2.bzl` | `eunit` | `@rules_dx//erlang/rules:defs.bzl` | `erlang_eunit_test` |
| `@rules_dotnet//dotnet:defs.bzl` | `fsharp_binary`, `fsharp_library`, `fsharp_test` | `@rules_dx_dotnet//fsharp/rules:defs.bzl` | `fsharp_binary`, `fsharp_library`, `fsharp_test` |
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
| `@rules_powershell//powershell:defs.bzl` | `pwsh_binary`, `pwsh_library`, `pwsh_test` | `@rules_dx//powershell/rules:defs.bzl` | `powershell_binary`, `powershell_library`, `powershell_test` |
| `@rules_py//py:defs.bzl` | `py_binary` | `@rules_dx//python/rules:defs.bzl` | `python_binary` |
| `@rules_py//py:defs.bzl` | `py_library` | `@rules_dx//python/rules:defs.bzl` | `python_library` |
| `@rules_py//py:defs.bzl` | `py_test`, `py_pytest_test` | `@rules_dx//python/rules:defs.bzl` | `python_test` |
| `@rules_ruby//ruby:defs.bzl` | `rb_binary`, `rb_library`, `rb_test` | `@rules_dx//ruby/rules:defs.bzl` | `ruby_binary`, `ruby_library`, `ruby_test` |
| `@rules_rust//rust:defs.bzl` | `rust_binary`, `rust_library`, `rust_proc_macro`, `rust_shared_library`, `rust_static_library`, `rust_test` | `@rules_dx//rust/rules:defs.bzl` | same names |
| `@rules_scala//scala:scala.bzl` | `scala_binary`, `scala_library`, `scala_test` | `@rules_dx//scala/rules:defs.bzl` | `scala_binary`, `scala_library`, `scala_test` |
| `@aspect_rules_ts//ts:defs.bzl` | `ts_project` | `@rules_dx//typescript/rules:defs.bzl` | `typescript_project` |
| `@aspect_rules_js//js:defs.bzl` | `js_test` | `@rules_dx//typescript/rules:defs.bzl` | `typescript_test` |
| `@aspect_rules_jest//jest:defs.bzl` | `jest_test` | `@rules_dx//typescript/rules:defs.bzl` | `typescript_test` |
| `@aspect_rules_js//js:defs.bzl` | `js_library` | `@rules_dx//vue/rules:defs.bzl` | `vue_library` |
| `@rules_zig//zig:defs.bzl` | `zig_binary`, `zig_library`, `zig_test` | `@rules_dx//zig/rules:defs.bzl` | `zig_binary`, `zig_library`, `zig_test` |
|  |  | `@rules_dx//ansible/rules:defs.bzl` | `ansible_library` |
|  |  | `@rules_dx//api_spec/rules:defs.bzl` | `api_spec_library` |
|  |  | `@rules_dx//appimage:defs.bzl` | `appimage_package` |
|  |  | `@rules_dx//asciidoc/rules:defs.bzl` | `asciidoc_library` |
|  |  | `@rules_dx//astro/rules:defs.bzl` | `astro_build` |
|  |  | `@rules_dx//astro/rules:defs.bzl` | `astro_library` |
|  |  | `@rules_dx//avro/rules:defs.bzl` | `avro_library` |
|  |  | `@rules_dx//bicep/rules:defs.bzl` | `bicep_library` |
|  |  | `@rules_dx//bicep/rules:defs.bzl` | `bicep_registry_module` |
|  |  | `@rules_dx//capnproto/rules:defs.bzl` | `capnp_library` |
|  |  | `@rules_dx//capnproto/rules:defs.bzl` | `std_import_path` |
|  |  | `@rules_dx//cc/rules:defs.bzl` | `cc_bench` |
|  |  | `@rules_dx//cc/rules:defs.bzl` | `cc_coverage_features` |
|  |  | `@rules_dx//cc/rules:defs.bzl` | `cc_coverage_mismatch` |
|  |  | `@rules_dx//cc/rules:defs.bzl` | `cc_coverage_probe` |
|  |  | `@rules_dx//cc/rules:defs.bzl` | `cc_coverage_settings` |
|  |  | `@rules_dx//cc/rules:defs.bzl` | `cc_coverage_unsupported` |
|  |  | `@rules_dx//cc/rules:defs.bzl` | `cc_fuzz_test` |
|  |  | `@rules_dx//cc/rules:defs.bzl` | `cc_param_file_features` |
|  |  | `@rules_dx//cloudformation/rules:defs.bzl` | `cloudformation_library` |
|  |  | `@rules_dx//cmake/rules:defs.bzl` | `cmake_library` |
|  |  | `@rules_dx//d/rules:defs.bzl` | `d_bench` |
|  |  | `@rules_dx//d/rules:defs.bzl` | `d_fuzz_test` |
|  |  | `@rules_dx//d/rules:defs.bzl` | `d_quality_srcs` |
|  |  | `@rules_dx//dart/rules:defs.bzl` | `dart_bench` |
|  |  | `@rules_dx//dart/rules:defs.bzl` | `dart_binary` |
|  |  | `@rules_dx//dart/rules:defs.bzl` | `dart_js_binary` |
|  |  | `@rules_dx//dart/rules:defs.bzl` | `dart_library` |
|  |  | `@rules_dx//dart/rules:defs.bzl` | `dart_root_package_name` |
|  |  | `@rules_dx//dart/rules:defs.bzl` | `dart_test` |
|  |  | `@rules_dx//dart/rules:defs.bzl` | `dart_wasm_binary` |
|  |  | `@rules_dx//deploy/rules:defs.bzl` | `dx_deployment` |
|  |  | `@rules_dx//deploy/tests/cross:defs.bzl` | `cross_deployment` |
|  |  | `@rules_dx//dockerfile/rules:defs.bzl` | `dockerfile_library` |
|  |  | `@rules_dx//elixir/rules:defs.bzl` | `app_sources` |
|  |  | `@rules_dx//elixir/rules:defs.bzl` | `cover_sources` |
|  |  | `@rules_dx//elixir/rules:defs.bzl` | `escript_headers` |
|  |  | `@rules_dx//elixir/rules:defs.bzl` | `helper_first` |
|  |  | `@rules_dx//elixir/rules:defs.bzl` | `nif_priv` |
|  |  | `@rules_dx//env:defs.bzl` | `env_host_filename` |
|  |  | `@rules_dx//env:defs.bzl` | `env_tool_record` |
|  |  | `@rules_dx//env:defs.bzl` | `env_tree_metadata` |
|  |  | `@rules_dx//env:defs.bzl` | `environment_config` |
|  |  | `@rules_dx//env:defs.bzl` | `environment_metadata` |
|  |  | `@rules_dx//env:defs.bzl` | `environment_tool` |
|  |  | `@rules_dx//env:defs.bzl` | `environment_tree` |
|  |  | `@rules_dx//erlang/rules:defs.bzl` | `app_sources` |
|  |  | `@rules_dx//erlang/rules:defs.bzl` | `ct_run_args` |
|  |  | `@rules_dx//erlang/rules:defs.bzl` | `ct_suite_source` |
|  |  | `@rules_dx//erlang/rules:defs.bzl` | `erlang_nif` |
|  |  | `@rules_dx//erlang/rules:defs.bzl` | `erlang_release` |
|  |  | `@rules_dx//erlang/rules:defs.bzl` | `escript_nif_message` |
|  |  | `@rules_dx//erlang/rules:defs.bzl` | `lcov_apps_dirs` |
|  |  | `@rules_dx//erlang/rules:defs.bzl` | `native_priv_files` |
|  |  | `@rules_dx//erlang/rules:defs.bzl` | `nif_linkopt_cases` |
|  |  | `@rules_dx//erlang/rules:defs.bzl` | `nif_priv_path` |
|  |  | `@rules_dx//erlang/rules:defs.bzl` | `otp_launch_descriptor` |
|  |  | `@rules_dx//erlang/rules:defs.bzl` | `otp_launcher_text` |
|  |  | `@rules_dx//erlang/rules:defs.bzl` | `otp_tool_binary_impl` |
|  |  | `@rules_dx//erlang/rules:defs.bzl` | `runfiles_path` |
|  |  | `@rules_dx_android//packaging:defs.bzl` | `abi_transition_settings` |
|  |  | `@rules_dx_android//packaging:defs.bzl` | `android_adb_install` |
|  |  | `@rules_dx_android//packaging:defs.bzl` | `android_adb_log` |
|  |  | `@rules_dx_android//packaging:defs.bzl` | `android_adb_start` |
|  |  | `@rules_dx_android//packaging:defs.bzl` | `android_apk_set` |
|  |  | `@rules_dx_android//packaging:defs.bzl` | `android_check` |
|  |  | `@rules_dx_android//packaging:defs.bzl` | `android_sign` |
|  |  | `@rules_dx_android//packaging:defs.bzl` | `rust_android_apk` |
|  |  | `@rules_dx_android//platforms:defs.bzl` | `api_floor` |
|  |  | `@rules_dx_android//platforms:defs.bzl` | `clang_target` |
|  |  | `@rules_dx_android//platforms:defs.bzl` | `constraint_list` |
|  |  | `@rules_dx_android//platforms:defs.bzl` | `cpu_for` |
|  |  | `@rules_dx_android//platforms:defs.bzl` | `effective_api_level` |
|  |  | `@rules_dx_android//platforms:defs.bzl` | `ndk_prebuilt_dir` |
|  |  | `@rules_dx_android//platforms:defs.bzl` | `platform_label` |
|  |  | `@rules_dx_android//platforms:defs.bzl` | `rust_triples` |
|  |  | `@rules_dx_android//platforms:defs.bzl` | `sysroot_api_dir` |
|  |  | `@rules_dx_android//platforms:defs.bzl` | `triple_for` |
|  |  | `@rules_dx_android//rules:defs.bzl` | `android_native_library` |
|  |  | `@rules_dx_apple//bundle:defs.bzl` | `archives` |
|  |  | `@rules_dx_apple//bundle:defs.bzl` | `check_cell` |
|  |  | `@rules_dx_apple//bundle:defs.bzl` | `display` |
|  |  | `@rules_dx_apple//bundle:defs.bzl` | `flat_resources` |
|  |  | `@rules_dx_apple//bundle:defs.bzl` | `framework_binaries` |
|  |  | `@rules_dx_apple//bundle:defs.bzl` | `label_text` |
|  |  | `@rules_dx_apple//bundle:defs.bzl` | `resolved_xcode` |
|  |  | `@rules_dx_apple//ios:defs.bzl` | `apple_ios_lib` |
|  |  | `@rules_dx_apple//ios:defs.bzl` | `cell_settings` |
|  |  | `@rules_dx_apple//ios:defs.bzl` | `deployment_env` |
|  |  | `@rules_dx_apple//ios:defs.bzl` | `framework_linkopts` |
|  |  | `@rules_dx_apple//ios:defs.bzl` | `ios_lib` |
|  |  | `@rules_dx_apple//ios:defs.bzl` | `ios_lib_cell` |
|  |  | `@rules_dx_apple//ios:defs.bzl` | `ios_platforms` |
|  |  | `@rules_dx_apple//ios:defs.bzl` | `platform_for` |
|  |  | `@rules_dx_apple//ios:defs.bzl` | `split_platform` |
|  |  | `@rules_dx_apple//macos:app.bzl` | `owner_relative_path` |
|  |  | `@rules_dx_apple//provisioning:defs.bzl` | `apple_provisioning` |
|  |  | `@rules_dx_apple//provisioning:defs.bzl` | `ceiling_for` |
|  |  | `@rules_dx_apple//provisioning:defs.bzl` | `deployment_floor` |
|  |  | `@rules_dx_apple//provisioning:defs.bzl` | `host_constraints` |
|  |  | `@rules_dx_apple//provisioning:defs.bzl` | `version_matches` |
|  |  | `@rules_dx_apple//signing:defs.bzl` | `apple_macos_notarize` |
|  |  | `@rules_dx_apple//signing:defs.bzl` | `apple_macos_sign` |
|  |  | `@rules_dx_cuda//rules:defs.bzl` | `cuda_binary` |
|  |  | `@rules_dx_cuda//rules:defs.bzl` | `cuda_gpu_env` |
|  |  | `@rules_dx_cuda//rules:defs.bzl` | `cuda_split_srcs` |
|  |  | `@rules_dx_cuda//rules:defs.bzl` | `cuda_target_compatible_with` |
|  |  | `@rules_dx_cuda//rules:defs.bzl` | `cuda_test` |
|  |  | `@rules_dx_cuda//tests:defs.bzl` | `with_clang_compiler` |
|  |  | `@rules_dx_dotnet//csharp/rules:defs.bzl` | `csharp_bench` |
|  |  | `@rules_dx_dotnet//csharp/rules:defs.bzl` | `csharp_fuzz_test` |
|  |  | `@rules_dx_dotnet//csharp/rules:defs.bzl` | `csharp_proto_library` |
|  |  | `@rules_dx_dotnet//fsharp/rules:defs.bzl` | `fsharp_bench` |
|  |  | `@rules_dx_dotnet//fsharp/rules:defs.bzl` | `fsharp_fuzz_test` |
|  |  | `@rules_dx_msix//:defs.bzl` | `compatible_with` |
|  |  | `@rules_dx_msix//:defs.bzl` | `msix_package` |
|  |  | `@rules_dx_msix//:defs.bzl` | `msix_sign` |
|  |  | `@rules_dx_msix//:defs.bzl` | `publisher_cn` |
|  |  | `@rules_dx//flatbuffers/rules:defs.bzl` | `flatbuffers_library` |
|  |  | `@rules_dx//flatbuffers/rules:defs.bzl` | `resolve_root` |
|  |  | `@rules_dx//flutter/rules:defs.bzl` | `flutter_android_app` |
|  |  | `@rules_dx//flutter/rules:defs.bzl` | `flutter_application` |
|  |  | `@rules_dx//flutter/rules:defs.bzl` | `flutter_ios_app` |
|  |  | `@rules_dx//flutter/rules:defs.bzl` | `flutter_library` |
|  |  | `@rules_dx//flutter/rules:defs.bzl` | `flutter_test` |
|  |  | `@rules_dx//flutter/rules:defs.bzl` | `flutter_web_app` |
|  |  | `@rules_dx//flutter/rules:defs.bzl` | `flutter_web_bundle` |
|  |  | `@rules_dx//fortran/rules:defs.bzl` | `fortran_binary` |
|  |  | `@rules_dx//fortran/rules:defs.bzl` | `fortran_library` |
|  |  | `@rules_dx//fortran/rules:defs.bzl` | `fortran_test` |
|  |  | `@rules_dx//fortran/rules:defs.bzl` | `library_name` |
|  |  | `@rules_dx//go/rules:defs.bzl` | `go_bench` |
|  |  | `@rules_dx//go/rules:defs.bzl` | `go_fuzz_test` |
|  |  | `@rules_dx//haskell/rules:defs.bzl` | `haskell_bench` |
|  |  | `@rules_dx//haskell/rules:defs.bzl` | `with_gmp` |
|  |  | `@rules_dx//helm/rules:defs.bzl` | `helm_chart` |
|  |  | `@rules_dx//java/rules:defs.bzl` | `java_bench` |
|  |  | `@rules_dx//java/rules:defs.bzl` | `java_fuzz_test` |
|  |  | `@rules_dx//java/rules:defs.bzl` | `java_jmh_test` |
|  |  | `@rules_dx//javascript/rules:defs.bzl` | `javascript_bench` |
|  |  | `@rules_dx//javascript/rules:defs.bzl` | `javascript_binary_upstream_data` |
|  |  | `@rules_dx//javascript/rules:defs.bzl` | `javascript_fuzz_test` |
|  |  | `@rules_dx//javascript/rules:defs.bzl` | `javascript_test_rejection` |
|  |  | `@rules_dx//javascript/rules:defs.bzl` | `javascript_wasm_bindgen_library` |
|  |  | `@rules_dx//julia/rules:defs.bzl` | `julia_standalone_binary` |
|  |  | `@rules_dx//kotlin/rules:defs.bzl` | `kotlin_bench` |
|  |  | `@rules_dx//kotlin/rules:defs.bzl` | `kotlin_fuzz_test` |
|  |  | `@rules_dx//kotlin/rules:defs.bzl` | `kotlin_warnings_as_errors_opts` |
|  |  | `@rules_dx//kubernetes/rules:defs.bzl` | `kubernetes_library` |
|  |  | `@rules_dx//kubernetes/rules:defs.bzl` | `kubernetes_schemas` |
|  |  | `@rules_dx//libs/exec_os:defs.bzl` | `exec_os` |
|  |  | `@rules_dx//libs/launcher:defs.bzl` | `native_launch_descriptor_path` |
|  |  | `@rules_dx//libs/launcher:defs.bzl` | `target_is_windows` |
|  |  | `@rules_dx//libs/launcher:defs.bzl` | `write_native_launcher` |
|  |  | `@rules_dx//libs/starlark:defs.bzl` | `display_label` |
|  |  | `@rules_dx//libs/starlark:defs.bzl` | `expect_contains` |
|  |  | `@rules_dx//libs/starlark:defs.bzl` | `expect_equal` |
|  |  | `@rules_dx//libs/starlark:defs.bzl` | `expect_false` |
|  |  | `@rules_dx//libs/starlark:defs.bzl` | `expect_match` |
|  |  | `@rules_dx//libs/starlark:defs.bzl` | `expect_true` |
|  |  | `@rules_dx//libs/starlark:defs.bzl` | `fake_file` |
|  |  | `@rules_dx//libs/starlark:defs.bzl` | `label_string` |
|  |  | `@rules_dx//libs/starlark:defs.bzl` | `starlark_test` |
|  |  | `@rules_dx//lua/rules:defs.bzl` | `lua_binary` |
|  |  | `@rules_dx//lua/rules:defs.bzl` | `lua_library` |
|  |  | `@rules_dx//lua/rules:defs.bzl` | `lua_test` |
|  |  | `@rules_dx//make/rules:defs.bzl` | `make_library` |
|  |  | `@rules_dx//mdx/rules:defs.bzl` | `mdx_build` |
|  |  | `@rules_dx//mdx/rules:defs.bzl` | `mdx_library` |
|  |  | `@rules_dx//ocaml/rules:defs.bzl` | `launcher_script` |
|  |  | `@rules_dx//ocaml/rules:defs.bzl` | `ocaml_binary` |
|  |  | `@rules_dx//ocaml/rules:defs.bzl` | `ocaml_library` |
|  |  | `@rules_dx//ocaml/rules:defs.bzl` | `ocaml_module` |
|  |  | `@rules_dx//ocaml/rules:defs.bzl` | `ocaml_signature` |
|  |  | `@rules_dx//ocaml/rules:defs.bzl` | `ocaml_test` |
|  |  | `@rules_dx//ocaml/rules:defs.bzl` | `owned_sources` |
|  |  | `@rules_dx//ocaml/rules:defs.bzl` | `runfiles_path` |
|  |  | `@rules_dx//ocaml/rules:defs.bzl` | `toolchain_choice` |
|  |  | `@rules_dx//ocaml/rules:defs.bzl` | `toolchain_settings` |
|  |  | `@rules_dx//ocaml/tests/native:defs.bzl` | `sys_cell_test` |
|  |  | `@rules_dx//perl/rules:defs.bzl` | `perl_bench` |
|  |  | `@rules_dx//php/rules:defs.bzl` | `php_bench` |
|  |  | `@rules_dx//php/rules:defs.bzl` | `php_binary` |
|  |  | `@rules_dx//php/rules:defs.bzl` | `php_library` |
|  |  | `@rules_dx//php/rules:defs.bzl` | `php_test` |
|  |  | `@rules_dx//python/rules:defs.bzl` | `python_bench` |
|  |  | `@rules_dx//python/rules:defs.bzl` | `python_fuzz_test` |
|  |  | `@rules_dx//python/rules:defs.bzl` | `python_grpc_library` |
|  |  | `@rules_dx//python/rules:defs.bzl` | `python_main` |
|  |  | `@rules_dx//python/rules:defs.bzl` | `python_main_rejection` |
|  |  | `@rules_dx//python/rules:defs.bzl` | `python_test_rejection` |
|  |  | `@rules_dx//qt/rules:defs.bzl` | `qt_bundle` |
|  |  | `@rules_dx//qt/rules:defs.bzl` | `qt_bundle_members` |
|  |  | `@rules_dx//qt/rules:defs.bzl` | `qt_cc_binary` |
|  |  | `@rules_dx//qt/rules:defs.bzl` | `qt_cc_library` |
|  |  | `@rules_dx//qt/rules:defs.bzl` | `qt_cc_test` |
|  |  | `@rules_dx//qt/rules:defs.bzl` | `qt_consumer_identity` |
|  |  | `@rules_dx//qt/rules:defs.bzl` | `qt_framework_path` |
|  |  | `@rules_dx//qt/rules:defs.bzl` | `qt_frameworks` |
|  |  | `@rules_dx//qt/rules:defs.bzl` | `qt_identifier` |
|  |  | `@rules_dx//qt/rules:defs.bzl` | `qt_library_stem` |
|  |  | `@rules_dx//qt/rules:defs.bzl` | `qt_module_path` |
|  |  | `@rules_dx//qt/rules:defs.bzl` | `qt_os_supplied_libraries` |
|  |  | `@rules_dx//qt/rules:defs.bzl` | `qt_package_path` |
|  |  | `@rules_dx//qt/rules:defs.bzl` | `qt_qml_module` |
|  |  | `@rules_dx//qt/rules:defs.bzl` | `qt_qmldir` |
|  |  | `@rules_dx//qt/rules:defs.bzl` | `qt_qrc` |
|  |  | `@rules_dx//qt/rules:defs.bzl` | `qt_runtime_members` |
|  |  | `@rules_dx//qt/rules:defs.bzl` | `qt_rust_cc_library` |
|  |  | `@rules_dx//qt/rules:defs.bzl` | `qt_rust_runtime` |
|  |  | `@rules_dx//qt/rules:defs.bzl` | `qt_version_parts` |
|  |  | `@rules_dx//quality/tools/javascript:defs.bzl` | `js_tool` |
|  |  | `@rules_dx//quality/tools/javascript:defs.bzl` | `js_tool_binary` |
|  |  | `@rules_dx//rego/rules:defs.bzl` | `rego_library` |
|  |  | `@rules_dx//rego/rules:defs.bzl` | `rego_test` |
|  |  | `@rules_dx//rst/rules:defs.bzl` | `rst_library` |
|  |  | `@rules_dx//ruby/rules:defs.bzl` | `ruby_bench` |
|  |  | `@rules_dx//ruby/rules:defs.bzl` | `ruby_coverage_lib_dirs` |
|  |  | `@rules_dx//rust/rules:defs.bzl` | `rust_bench` |
|  |  | `@rules_dx//rust/rules:defs.bzl` | `rust_binary` |
|  |  | `@rules_dx//rust/rules:defs.bzl` | `rust_fuzz_test` |
|  |  | `@rules_dx//rust/rules:defs.bzl` | `rust_library` |
|  |  | `@rules_dx//rust/rules:defs.bzl` | `rust_proc_macro` |
|  |  | `@rules_dx//rust/rules:defs.bzl` | `rust_shared_library` |
|  |  | `@rules_dx//rust/rules:defs.bzl` | `rust_static_library` |
|  |  | `@rules_dx//rust/rules:defs.bzl` | `rust_test` |
|  |  | `@rules_dx//scala/rules:defs.bzl` | `scala_bench` |
|  |  | `@rules_dx//scala/rules:defs.bzl` | `scala_fuzz_test` |
|  |  | `@rules_dx//sql/rules:defs.bzl` | `sql_library` |
|  |  | `@rules_dx//svelte/rules:defs.bzl` | `svelte_build` |
|  |  | `@rules_dx//svelte/rules:defs.bzl` | `svelte_library` |
|  |  | `@rules_dx//swift/rules:defs.bzl` | `swift_bench` |
|  |  | `@rules_dx//swift/rules:defs.bzl` | `swift_binary` |
|  |  | `@rules_dx//swift/rules:defs.bzl` | `swift_fuzz_test` |
|  |  | `@rules_dx//swift/rules:defs.bzl` | `swift_library` |
|  |  | `@rules_dx//swift/rules:defs.bzl` | `swift_test` |
|  |  | `@rules_dx//terraform/rules:defs.bzl` | `collect_providers` |
|  |  | `@rules_dx//terraform/rules:defs.bzl` | `terraform_library` |
|  |  | `@rules_dx//thrift/rules:defs.bzl` | `thrift_library` |
|  |  | `@rules_dx//tools/docs/components:defs.bzl` | `astro_doc_shard` |
|  |  | `@rules_dx//tools/docs/components:defs.bzl` | `component_doc_shard` |
|  |  | `@rules_dx//tools/docs/components:defs.bzl` | `mdx_doc_shard` |
|  |  | `@rules_dx//tools/docs/components:defs.bzl` | `svelte_doc_shard` |
|  |  | `@rules_dx//tools/docs/components:defs.bzl` | `vue_doc_shard` |
|  |  | `@rules_dx//tools/docs/dotnet:defs.bzl` | `csharp_doc_shard` |
|  |  | `@rules_dx//tools/docs/dotnet:defs.bzl` | `fsharp_doc_shard` |
|  |  | `@rules_dx//tools/docs/go:defs.bzl` | `docs_go_shard` |
|  |  | `@rules_dx//tools/docs/go:defs.bzl` | `go_doc_shard` |
|  |  | `@rules_dx//tools/docs/go:defs.bzl` | `go_shard_action` |
|  |  | `@rules_dx//tools/docs/jvm:defs.bzl` | `java_doc_shard` |
|  |  | `@rules_dx//tools/docs/jvm:defs.bzl` | `kotlin_doc_shard` |
|  |  | `@rules_dx//tools/docs/jvm:defs.bzl` | `scala_doc_shard` |
|  |  | `@rules_dx//tools/docs/powershell:defs.bzl` | `powershell_doc_shard` |
|  |  | `@rules_dx//tools/docs/powershell:defs.bzl` | `powershell_shard_action` |
|  |  | `@rules_dx//tools/docs/python:defs.bzl` | `docs_python_shard` |
|  |  | `@rules_dx//tools/docs/python:defs.bzl` | `python_doc_shard` |
|  |  | `@rules_dx//tools/docs/python:defs.bzl` | `python_docs` |
|  |  | `@rules_dx//tools/docs/ruby:defs.bzl` | `docs_ruby_shard` |
|  |  | `@rules_dx//tools/docs/ruby:defs.bzl` | `ruby_doc_runtime` |
|  |  | `@rules_dx//tools/docs/ruby:defs.bzl` | `ruby_doc_shard` |
|  |  | `@rules_dx//tools/docs/ruby:defs.bzl` | `ruby_shard_action` |
|  |  | `@rules_dx//tools/docs/rust:defs.bzl` | `rust_shard_action` |
|  |  | `@rules_dx//tools/docs/typescript:defs.bzl` | `docs_typescript_shard` |
|  |  | `@rules_dx//tools/docs/typescript:defs.bzl` | `typescript_doc_shard` |
|  |  | `@rules_dx//tools/testing/hermetic:defs.bzl` | `hermetic_manifest` |
|  |  | `@rules_dx//tools/testing/hermetic:defs.bzl` | `hermetic_qualification` |
|  |  | `@rules_dx//tools/testing/hermetic:defs.bzl` | `nested_bazel_test` |
|  |  | `@rules_dx//tools/testing/starters:defs.bzl` | `starter_consumer_test` |
|  |  | `@rules_dx//typescript/rules:defs.bzl` | `dx_scoped_tsconfig_out` |
|  |  | `@rules_dx//typescript/rules:defs.bzl` | `dx_tsconfig_content` |
|  |  | `@rules_dx//typescript/rules:defs.bzl` | `dx_tsconfig_is_source` |
|  |  | `@rules_dx//typescript/rules:defs.bzl` | `dx_tsconfig_relpath` |
|  |  | `@rules_dx//typescript/rules:defs.bzl` | `typescript_bench` |
|  |  | `@rules_dx//typescript/rules:defs.bzl` | `typescript_declaration_srcs_rejection` |
|  |  | `@rules_dx//typescript/rules:defs.bzl` | `typescript_empty_project_rejection` |
|  |  | `@rules_dx//typescript/rules:defs.bzl` | `typescript_fuzz_test` |
|  |  | `@rules_dx//typescript/rules:defs.bzl` | `typescript_playwright_test` |
|  |  | `@rules_dx//typescript/rules:defs.bzl` | `typescript_proto_library` |
|  |  | `@rules_dx//typescript/rules:defs.bzl` | `typescript_scoped_tsconfig_rejection` |
|  |  | `@rules_dx//typescript/rules:defs.bzl` | `typescript_srcs_rejection` |
|  |  | `@rules_dx//typescript/rules:defs.bzl` | `typescript_test_rejection` |
|  |  | `@rules_dx//typescript/rules:defs.bzl` | `typescript_transpiler` |
|  |  | `@rules_dx//typescript/rules:defs.bzl` | `typescript_upstream_srcs` |
|  |  | `@rules_dx//vue/rules:defs.bzl` | `vue_build` |
|  |  | `@rules_dx//zig/rules:defs.bzl` | `zig_bench` |
|  |  | `@rules_dx//zig/rules:defs.bzl` | `zig_import_name` |

Packaging and deploy:

- `@rules_dx_android//packaging:defs.bzl`: APKs, App Bundles and adb.
- `@rules_dx//appimage:defs.bzl`: AppImage bundles.
- `@rules_dx_apple//ios:device.bzl`: signed iOS IPA export.
- `@rules_dx_apple//provisioning:defs.bzl`: Apple provisioning.
- `@rules_dx_apple//provisioning:probe.bzl`: Apple toolchain probe.
- `@rules_dx_apple//signing:defs.bzl`: macOS app signing and notarization.
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
- `@rules_dx//deploy/rules:pub.bzl`: pub.dev publish.
- `@rules_dx//deploy/rules:pypi.bzl`: PyPI publish.
- `@rules_dx_msix//:defs.bzl`: MSIX packages.

SDK and toolchain extensions:

- `@rules_dx_android//sdk:extensions.bzl`: Android SDK.
- `@rules_dx_android//toolchain:repos.bzl`: Android NDK toolchain.
- `@rules_dx_apple//ios:simulator.bzl`: iOS simulator.
- `@rules_dx_cuda//toolchain:repos.bzl`: CUDA license acceptance.
- `@rules_dx//elixir/rules:hex.bzl`: Hex packages for Elixir.
- `@rules_dx//powershell/toolchain:gallery.bzl`: PowerShell Gallery modules.
- `@rules_dx_msix//:extensions.bzl`: MSIX tools.
- `@rules_dx//qt/sdk:extensions.bzl`: Qt SDK.

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

Every other file stays private.
A private file is not a dependency.
The Bazel version in `.bazelversion` is the tested baseline.
Bazel 9.2.0 or later is required.

rules_dx does not provide debugger acquisition, launch or attach integration,
Debug Adapter Protocol adapters, or generated debugger configurations. These
are unsupported and not planned. Debug build modes and editor
language-server settings do not imply debugger support. You can set up
external debuggers yourself.
