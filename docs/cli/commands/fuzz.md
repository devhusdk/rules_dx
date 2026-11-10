# `dx fuzz`

Runs the fuzz targets in scope for a time budget and keeps their crashes under `.dx/local/fuzz`.

```sh
dx fuzz //pkg:parser
dx fuzz --budget 5m //pkg/...
dx fuzz --budget 90s --jobs 4
```

```text
dx fuzz [--output json] [--here] [--budget <duration>] [--jobs <n>] [scope ...]
```

A fuzz target is a rule tagged `dx-fuzz` with a runner named `<name>_run` next to it. Targets without the tag are skipped. Targets run one after another.

Every `<lang>_fuzz_test` macro has the same shape. `<name>` is the manual `dx-fuzz` target dx runs, the program is a private `<name>_bin`, and `<name>_run` is the runner. `args` and `target_compatible_with` go to `<name>`; `go_fuzz_test` also takes `size`, `timeout`, `env`, `data`, `flaky` and `shard_count` there. The other attributes go to `<name>_bin`. `tags` are merged with `dx-fuzz` and `manual`.

Go: `go_fuzz_test(name, target = "FuzzParse", srcs = ..., ...)` from `//go/rules:defs.bzl` declares the test, tags it and adds the `<name>_run` runner.

D: `d_fuzz_test(name, srcs)` from `//d/rules:defs.bzl` builds with LDC's `-fsanitize=fuzzer,address` and the pinned LLVM's libFuzzer. `srcs` define `LLVMFuzzerTestOneInput`, for example through `ldc.libfuzzer`.

Python: `python_fuzz_test(name, srcs, main, deps = ["@pypi//atheris"])` from `//python/rules:defs.bzl` declares a binary that calls `atheris.Setup` and `atheris.Fuzz`, tags it and adds the `<name>_run` runner. Manual and Linux x86_64 only: there is no atheris wheel for Windows.

JavaScript and TypeScript: `javascript_fuzz_test(name, srcs, node_modules)` from `//javascript/rules:defs.bzl` and `typescript_fuzz_test` from `//typescript/rules:defs.bzl` run the `fuzz(data)` function that the first source (or `main`) exports with [Jazzer.js](https://github.com/CodeIntelligenceTesting/jazzer.js), tag it and add the `<name>_run` runner. List `@jazzer.js/core` in `package.json` so `node_modules` links it. pnpm needs `packageExtensions` that add `@babel/core` and `@babel/types` to `@jazzer.js/bug-detectors` and `@babel/types` to `@jazzer.js/instrumentor`, which use them without declaring them. Jazzer.js ships native add-ons for Linux, macOS arm64 and x64, and Windows x64.

Rust: `rust_fuzz_test(name, srcs, libfuzzer_sys = ..., crate_root = ...)` from `//rust/rules:defs.bzl` builds a `libfuzzer-sys` binary with sancov and AddressSanitizer using `RUSTC_BOOTSTRAP=1` and the pinned LLVM's libFuzzer, tags it and adds the `<name>_run` runner. The source is `#![no_main]` and calls `fuzz_target!`. `libfuzzer_sys = "@<your_hub>//:libfuzzer-sys"` names the `libfuzzer-sys` crate in your own hub, which also lets you use `arbitrary`. Tested on linux x86_64 only.

C#: `csharp_fuzz_test(name, srcs, deps)` from `//csharp/rules:defs.bzl` declares a harness binary that calls `Fuzzer.LibFuzzer.Run` from SharpFuzz, tags it and adds the `<name>_run` runner. The macro instruments each library in `deps` with SharpFuzz.CommandLine at build time; NuGet packages stay uninstrumented. `<name>` is the harness, and the runner starts it from the libfuzzer-dotnet driver that rules_dx builds from a pinned source archive. Linux only for now.

F#: `fsharp_fuzz_test(name, srcs, deps)` from `//fsharp/rules:defs.bzl` works like `csharp_fuzz_test` with an F# harness. Linux only for now.

Java, Kotlin and Scala: `java_fuzz_test`, `kotlin_fuzz_test` and `scala_fuzz_test` from `//java/rules:defs.bzl`, `//kotlin/rules:defs.bzl` and `//scala/rules:defs.bzl` build a Jazzer binary, tag it and add the `<name>_run` runner. Pass `target_class`, a class with a static `fuzzerTestOneInput(byte[])`: a Kotlin `object` method marked `@JvmStatic`, or a top-level Scala `object` method. Linux runs are tested; Jazzer comes from rules_dx's pinned Maven artifacts.

OCaml: `ocaml_fuzz_test(name, main, deps)` from `//ocaml/rules:defs.bzl` runs a Crowbar test in quickcheck mode. It is manual, has no `dx-fuzz` tag or `<name>_run` runner, and `dx fuzz` does not run it yet (#10221). Run it with `bazel test`.

Zig and Haskell have no fuzz macro. Zig's fuzzer runs only under `zig build --fuzz`, and Haskell's harness cannot run hermetically.

| Flag | What it does |
| --- | --- |
| `--budget <duration>` | Time budget per target: `90`, `90s`, `5m` or `1h`. Default: `60s`. |
| `--jobs <n>` | Jobs each runner may use. Passed to the runner as `--jobs <n>`. |
| `--output json` | Write NDJSON events on stdout instead of text. |
| `--here` | Run on the current directory tree. |
| `--workspace`, `-q`, `-v`, `--color`, `--build-slots`, `--build-slots-wait`, `--bazel-startup-option` | Global flags. |

Scopes: labels, patterns, files, or dirs. With no scope, dx fuzz runs `//...`.

## Runner contract

For each target dx runs:

```sh
bazel run //pkg:parser_run -- --time <seconds> --out .dx/local/fuzz/pkg/parser/
```

The runner resolves `--out` against `BUILD_WORKSPACE_DIRECTORY` and writes crash files named `crash-<hash>` there. It also accepts `--repro <file>` to replay one crash.

| Runner exit | Meaning |
| --- | --- |
| `0` | No crash. |
| `77` | A crash file was written. |
| Other | Tool error. |

## Output

A crash prints one line on stderr:

```text
dx: //pkg:parser crashed; reproduce with: dx run //pkg:parser_run -- --repro .dx/local/fuzz/pkg/parser/crash-<hash>
```

`/.dx/local/` is in the `.gitignore` that `dx init` writes.

With `--output json` dx writes `command_started`, one `notice` with code `fuzz_crash` per crash, and `command_finished`. The notice carries `target`, `runner`, `crash_path` and `repro`, the argv that replays the crash.

Reports: none.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | No target crashed. Also when no target in scope carries `dx-fuzz`. |
| `1` | A target crashed. |
| Runner's code | The fuzz runner exited nonzero without a crash, or Bazel failed. |
| `128`+N | A signal stopped the runner or dx. |
| `2` | Usage error. |
