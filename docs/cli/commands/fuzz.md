# `dx fuzz`

Use `dx fuzz` to run your fuzz targets for a set time and keep every crash they find.

A fuzz target feeds random input to your code to find crashes. In Bazel, a target is one thing Bazel builds, and a label names it, such as `//pkg:parser`: the package directory `pkg`, then the target name `parser`.

## Examples

Fuzz one target for the default 60 seconds:

```sh
dx fuzz //pkg:parser
```

Fuzz every target under `pkg/` for five minutes each:

```sh
dx fuzz --budget 5m //pkg/...
```

Fuzz every target in the repository for 90 seconds each, with 4 jobs per runner:

```sh
dx fuzz --budget 90s --jobs 4
```

Replay a crash dx found:

```sh
dx run //pkg:parser_run -- --repro .dx/local/fuzz/pkg/parser/crash-<hash>
```

## Usage

```text
dx fuzz [--output json] [--here] [--budget <duration>] [--jobs <n>] [scope ...]
```

## Flags

| Flag | What it does |
| --- | --- |
| `--budget <duration>` | Time budget per target, such as `90`, `90s`, `5m` or `1h`. A bare number is seconds. Default `60s`. |
| `--jobs <n>` | Jobs each fuzz runner may use. |
| `--here` | Run on the current directory tree. |
| `-o, --output text\|json` | Choose the output format. Default `text`. |
| `--workspace <dir>` | Run in this workspace. |
| `-q, --quiet` | Hide summaries. |
| `-v, --verbose` | Show more logs. |
| `--color auto\|always\|never` | Set color output. |
| `--build-slots auto\|<n>\|off` | Set build slots. |
| `--build-slots-wait <duration>` | Give up waiting for a build slot after this long. `0` fails at once. |
| `--bazel-startup-option <token>` | Pass one Bazel startup option. Repeatable. |

## Scopes

Pass labels, patterns such as `//pkg/...`, or workspace-relative files or directories. With no scope, `dx fuzz` runs `//...`, the whole repository. `--here` runs the current directory tree instead.

Only targets tagged `dx-fuzz` run. Targets under `[scope] exclude` in `dx.toml` are skipped. Targets run one after another.

## Declare a fuzz target

Declare fuzz targets in a `BUILD` file, the file in each package directory that lists its targets. Every `<lang>_fuzz_test` macro has the same shape:

- `<name>` is the `dx-fuzz` target dx runs.
- `<name>_bin` is the private program.
- `<name>_run` is the runner dx calls.

`args` and `target_compatible_with` go to `<name>`. `go_fuzz_test` also passes `size`, `timeout`, `env`, `data`, `flaky` and `shard_count` to `<name>`. Other attributes go to `<name>_bin`. Your `tags` are merged with `dx-fuzz`.

| Language | Macro | Load from | Notes |
| --- | --- | --- | --- |
| Go | `go_fuzz_test(name, target = "FuzzParse", srcs = ...)` | `@rules_dx//go:defs.bzl` | `target` is the fuzz function. |
| C and C++ | `cc_fuzz_test(name, srcs)` | `@rules_dx//cc:defs.bzl` | A libFuzzer binary with AddressSanitizer. `srcs` define `LLVMFuzzerTestOneInput`. |
| D | `d_fuzz_test(name, srcs)` | `@rules_dx//d:defs.bzl` | Builds with LDC's `-fsanitize=fuzzer,address` and the pinned LLVM's libFuzzer. `srcs` define `LLVMFuzzerTestOneInput`, for example through `ldc.libfuzzer`. |
| Rust | `rust_fuzz_test(name, srcs, libfuzzer_sys = ..., crate_root = ...)` | `@rules_dx//rust:defs.bzl` | A `libfuzzer-sys` binary with AddressSanitizer. The source is `#![no_main]` and calls `fuzz_target!`. `libfuzzer_sys = "@<your_hub>//:libfuzzer-sys"` names the crate in your own crate hub, which also lets you use `arbitrary`. Linux x86_64 only. |
| Python | `python_fuzz_test(name, srcs, main, deps = ["@pypi//atheris"])` | `@rules_dx//python:defs.bzl` | A binary that calls `atheris.Setup` and `atheris.Fuzz`. Linux x86_64 only. |
| JavaScript | `javascript_fuzz_test(name, srcs, node_modules)` | `@rules_dx//javascript:defs.bzl` | Runs the `fuzz(data)` function the first source, or `main`, exports with [Jazzer.js](https://github.com/CodeIntelligenceTesting/jazzer.js). |
| TypeScript | `typescript_fuzz_test(name, srcs, node_modules)` | `@rules_dx//typescript:defs.bzl` | Same as JavaScript. |
| C# | `csharp_fuzz_test(name, srcs, deps)` | `@rules_dx//csharp:defs.bzl` | A harness that calls `Fuzzer.LibFuzzer.Run` from SharpFuzz. Each library in `deps` is instrumented at build time. NuGet packages are not. Linux only. |
| F# | `fsharp_fuzz_test(name, srcs, deps)` | `@rules_dx//fsharp:defs.bzl` | Same as C# with an F# harness. Linux only. |
| OCaml | `ocaml_fuzz_test(name, main, deps)` | `@rules_dx//ocaml:defs.bzl` | Runs Crowbar with fresh seeds until the budget is up. |
| Java | `java_fuzz_test(name, srcs, target_class)` | `@rules_dx//java:defs.bzl` | A Jazzer binary. `target_class` has a static `fuzzerTestOneInput(byte[])`. |
| Kotlin | `kotlin_fuzz_test(name, srcs, target_class)` | `@rules_dx//kotlin:defs.bzl` | Same as Java. The method is on an `object` and marked `@JvmStatic`. |
| Scala | `scala_fuzz_test(name, srcs, target_class)` | `@rules_dx//scala:defs.bzl` | Same as Java. The method is on a top-level `object`. |

Zig and Haskell have no fuzz macro.

For Jazzer.js, list `@jazzer.js/core` in `package.json` so `node_modules` links it. With pnpm, add `packageExtensions` that give `@jazzer.js/bug-detectors` the deps `@babel/core` and `@babel/types`, and give `@jazzer.js/instrumentor` the dep `@babel/types`. Jazzer.js runs on Linux, macOS arm64 and x64, and Windows x64.

## Write your own runner

Any target tagged `dx-fuzz` with a runner named `<name>_run` next to it works. For each target, dx runs:

```sh
bazel run //pkg:parser_run -- --time <seconds> --out .dx/local/fuzz/pkg/parser/
```

With `--jobs <n>`, dx also passes the jobs count to the runner:

```sh
bazel run //pkg:parser_run -- --time <seconds> --out .dx/local/fuzz/pkg/parser/ --jobs <n>
```

The runner resolves the output directory against `BUILD_WORKSPACE_DIRECTORY` and writes each crash there as `crash-<hash>`. It must also replay one crash file:

```sh
bazel run //pkg:parser_run -- --repro <file>
```

| Runner exit | Meaning |
| --- | --- |
| `0` | No crash. |
| `77` | A crash file was written. |
| Other | Tool error. |

## Output

Text prints `Fuzzing <label> for <n>s` on stderr before each target. `--quiet` hides it. A crash prints one line on stderr:

```text
dx: //pkg:parser crashed; reproduce with: dx run //pkg:parser_run -- --repro .dx/local/fuzz/pkg/parser/crash-<hash>
```

A runner error prints `dx: fuzz_failed: <runner> exited <code>`. No `dx-fuzz` target in scope prints `dx: no dx-fuzz targets in scope`.

Crash files stay under `.dx/local/fuzz/<package>/<name>/`. The `.gitignore` that `dx init` writes ignores `/.dx/local/`.

With `--output json`, dx writes `command_started`, one `notice` with code `fuzz_crash` per crash, and `command_finished`. The notice has `target`, `runner`, `crash_path`, and `repro`, the command that replays the crash.

Reports: none.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | No target crashed, or no target in scope is tagged `dx-fuzz`. |
| `1` | A target crashed. |
| `2` | Usage or scope error, such as a bad `--budget`. |
| The runner's code | The runner exited nonzero without a crash, or Bazel failed. |
| `128`+N | Signal N stopped a runner or dx. |

dx runs every target and exits with the code of the first one that failed.
