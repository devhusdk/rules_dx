# Rust rules

`defs.bzl` wraps the `rules_rust` rules. One `rust_binary` builds native,
browser, and WASI outputs. The target platform picks the output.

```starlark
load(
    "@rules_dx//rust/rules:defs.bzl",
    "rust_binary",
    "rust_test",
)

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

## Benchmarks

`rust_benchmark(name, srcs, deps, ...)` is a `rust_test` built with the
`dx_bench` feature and tagged `dx-bench` and `manual`. It runs the binary with
`--bench --output-format bencher`, sets `CRITERION_HOME` under the undeclared
outputs, and writes the `test NAME ... bench: N ns/iter (+/- M)` lines to
`dx-bench.json`. `dx bench` runs it. The test is built without the libtest harness
(`use_libtest_harness = False`), so criterion's own `main` runs; pass
`use_libtest_harness = True` for nightly `#[bench]` functions.
`//rust/tests/fixtures/bench` is a criterion example.

## Native

```sh
bazel build //pkg:tool
```

On a libstdc++ platform, such as `@rules_dx//qt/platforms:linux_x86_64`,
crates and their dependencies build without the static C++ runtime.
Binaries and tests link LLVM libunwind. The private `<name>_upstream`
targets are incompatible there; build `<name>`.

## Target triples

Add the Wasm triples for browser and WASI builds to your root `MODULE.bazel`:

```python
bazel_dep(name = "rules_rust", version = "0.75.0")

rust = use_extension("@rules_rust//rust:extensions.bzl", "rust")
rust.toolchain(
    edition = "2021",
    extra_target_triples = ["wasm32-unknown-unknown", "wasm32-wasip1"],
    rustfmt_version = "1.99.1",
    versions = ["1.99.1"],
)
use_repo(rust, "rust_toolchains")
```

List only the triples you build.

## Browser

`rust_binary(wasm = "browser")` builds the binary for `wasm32-unknown-unknown`
and emits the JavaScript bindings: `<name>.js`, `<name>_bg.wasm`, and the
declarations. Guard browser-only code with
`#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]`.

`rust_test(srcs = ["browser_test.rs"], wasm = "browser")` builds a test crate
and runs its `#[wasm_bindgen_test]` cases in a browser that Bazel fetches. The
file needs a `fn main`. One file becomes `crate_root`. `deps`,
`proc_macro_deps` and `crate_root` go to the test crate.

`rust_test(crate = ":tool_web", wasm = "browser")` runs the cases of an
existing `rust_binary(wasm = "browser")` instead.
It runs on linux x86_64 with Firefox, where the host needs GTK 3, and only
runs locally. Other platforms skip it as incompatible. Wasm builds and
browser tests run without coverage instrumentation, so `bazel coverage` runs
them but reports no Wasm coverage.

For `nodejs` output, bindgen flags, or a custom toolchain, load
`rust_wasm_bindgen` from `@rules_rust_wasm_bindgen//:defs.bzl`.

## WASI

`rust_binary(wasm = "wasi")` builds for `wasm32-wasip1` and outputs one
`.wasm` file, without coverage. Building it fetches no runtime. A plain
`rust_binary` also builds for WASI with:

```sh
bazel build --platforms=@rules_rust//rust/platform:wasip1 //pkg:tool
```

`rust_test(wasm = "wasi")` builds the test crate for `wasm32-wasip1`,
without coverage, and runs it under Wasmtime. `srcs` or `crate` must
build one test module. The guest gets only the declared `args`, `env`
entries, and `preopens`. `--test_filter` reaches the guest's test
harness. `env` and `preopens` guest paths are literal.

| Attribute | Meaning |
| --- | --- |
| `wasm` | Set to `"wasi"`. |
| `args` | Guest arguments. |
| `env` | Guest environment entries. |
| `preopens` | Guest directory mapped to one host file, staged alone. |
| `runtime` | Executable that acts like `wasmtime run`. Default Wasmtime. Needs `wasm = "wasi"`. |

To check a built `wasm = "wasi"` binary from outside, use a native
`rust_test`. Put `@rules_dx//wasi/runtime:wasmtime` and the module in
`data` and `@rules_dx//wasi/testing:wasi_run` in `deps`. Pass their
`$(rootpath)` values in `env`. Then call
`wasi_run::run(runtime, module, args, env, preopens) -> Output` and
assert on the exit code, stdout, and stderr. Preopens map a guest
directory to one host file.

A custom `runtime` receives `DX_WASI_MANAGED_RUNTIME`, the path of the
default Wasmtime, so it can delegate. Proc macros and build scripts stay
on the execution platform. `rust_test` runs unit tests natively.
