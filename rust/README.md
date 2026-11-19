# Rust

This directory helps you build, run and test Rust code with rules_dx.

Load its rules in a `BUILD.bazel` file, the file that declares what Bazel
builds, from `@rules_dx//rust:defs.bzl`.
`extensions.bzl` holds the module extension: the part you call from
`MODULE.bazel` to set up toolchains and dependencies.
The tests live in `tests/`. The [Rust page](../docs/languages/rust.md) shows
setup, every rule and the tools dx runs.

Run the Rust tests from a rules_dx checkout:

```sh
tools/dx test //rust/...
```

## Bench and fuzz

`rust_bench` runs a criterion benchmark for `dx bench`:

```python
load("@rules_dx//rust:defs.bzl", "rust_bench")

rust_bench(
    name = "parse_bench",
    srcs = ["parse_bench.rs"],
    deps = ["@crates//:criterion"],
)
```

It is a `rust_test` built with the `dx_bench` crate feature and tagged
`dx-bench`. It builds without the libtest harness, so criterion's own `main`
runs. For nightly `#[bench]` functions, set `use_libtest_harness = True`.

The test runs the binary with `--bench --output-format bencher` and keeps
criterion's files (`CRITERION_HOME`) in the undeclared outputs. It turns each
`test NAME ... bench: N ns/iter (+/- M)` line into a row of `dx-bench.json`.
`rust/tests/fixtures/bench` is a full example.

## Toolchain

### Edition

Targets build with Rust edition 2024. To use another edition, set `edition`
on the target, for example `edition = "2021"`.

### WebAssembly

One `rust_binary` builds native, browser or WASI output. The `wasm` attribute
picks it: leave it out for a native binary, set `"browser"` for a browser
module, or `"wasi"` for a WASI module.

```python
load("@rules_dx//rust:defs.bzl", "rust_binary", "rust_test")

rust_binary(
    name = "tool",
    srcs = ["main.rs"],
)

rust_binary(
    name = "tool_web",
    srcs = ["main.rs"],
    wasm = "browser",
)

rust_binary(
    name = "tool_wasi",
    srcs = ["main.rs"],
    wasm = "wasi",
)

rust_test(
    name = "tool_wasi_test",
    srcs = ["tool_test.rs"],
    env = {"TOOL_MODE": "check"},
    preopens = {"/data": ":input.txt"},
    wasm = "wasi",
)
```

Wasm builds and
browser tests run without coverage instrumentation, so `bazel coverage` runs
them but reports no Wasm coverage.

### Native

Build the native binary as usual:

```sh
bazel build //pkg:tool
```

On a libstdc++ platform, such as `@rules_dx//qt:linux_x86_64`, crates build
without the static C++ runtime, and binaries and tests link LLVM libunwind.
Build `<name>` there; the `<name>_upstream` targets are incompatible.

### Browser

`rust_binary(wasm = "browser")` builds for `wasm32-unknown-unknown` and emits
the JavaScript bindings: `<name>.js`, `<name>_bg.wasm` and the type
declarations. Guard browser-only code with
`#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]`. The
`wasm_bindgen` crate is already in `deps`, so `use wasm_bindgen::prelude::*`
works without another `bazel_dep`.

To test browser code, write `#[wasm_bindgen_test]` cases in a file with a
`fn main`:

```python
rust_test(
    name = "browser_test",
    srcs = ["browser_test.rs"],
    wasm = "browser",
)
```

The file becomes `crate_root`. `deps`, `proc_macro_deps` and `crate_root` go
to the test crate. To run the cases of an existing browser binary instead, set
`crate = ":tool_web"` in place of `srcs`.

Browser tests run in a Firefox that Bazel fetches, on Linux x86_64 only, and
never remotely. The host needs GTK 3. Other platforms skip them as
incompatible.

For `nodejs` output, wasm-bindgen flags or your own toolchain, load
`rust_wasm_bindgen` from `@rules_rust_wasm_bindgen//:defs.bzl`.

### WASI

`rust_binary(wasm = "wasi")` builds for `wasm32-wasip1` and outputs one
`.wasm` file. Building it fetches no runtime. A plain `rust_binary` also
builds for WASI with:

```sh
bazel build --platforms=@rules_rust//rust/platform:wasip1 //pkg:tool
```

`rust_test(wasm = "wasi")` builds the test crate for `wasm32-wasip1` and runs
it under Wasmtime. `srcs` or `crate` must build one test module. The guest
sees only the `args`, `env` entries and `preopens` you declare, and
`--test_filter` reaches its test harness. Guest paths in `env` and `preopens`
are used as written.

| Attribute | Meaning |
| --- | --- |
| `wasm` | Set to `"wasi"`. |
| `args` | Guest arguments. |
| `env` | Guest environment entries. |
| `preopens` | Guest directory mapped to one host file, staged alone. |
| `runtime` | Executable that acts like `wasmtime run`. Default Wasmtime. Needs `wasm = "wasi"`. |

A custom `runtime` gets `DX_WASI_MANAGED_RUNTIME`, the path of the default
Wasmtime, so it can hand off to it. Proc macros and build scripts still build
for the execution platform, and `rust_test` without `wasm` runs unit tests
natively.

To check a built WASI binary from the outside, write a native `rust_test`:

1. Put `@rules_dx//rust:wasmtime` and the module in `data`.
2. Put `@rules_dx//rust:wasi_run` in `deps`.
3. Pass their `$(rootpath)` values in `env`.
4. Call `wasi_run::run(runtime, module, args, env, preopens) -> Output` and
   assert on the exit code, stdout and stderr.

## Platforms

### Target triples

`bazel_dep(name = "rules_dx")` brings the `wasm32-unknown-unknown` and
`wasm32-wasip1` toolchains. Browser and WASI builds need no `rust.toolchain`
call in your `MODULE.bazel`.
