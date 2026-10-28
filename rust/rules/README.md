# Rust rules

`defs.bzl` wraps the `rules_rust` rules. One `rust_binary` builds native,
browser, and WASI outputs. The target platform picks the output.

```starlark
load(
    "@rules_dx//rust/rules:defs.bzl",
    "rust_binary",
    "rust_wasi_module",
    "rust_wasi_test",
    "rust_wasm_bindgen",
)

rust_binary(
    name = "tool",
    srcs = ["main.rs"],
)

rust_wasm_bindgen(
    name = "tool_web",
    target = "web",
    wasm_file = ":tool_upstream",
)

rust_wasi_module(
    name = "tool_wasi",
    binary = ":tool",
)

rust_wasi_test(
    name = "tool_wasi_test",
    binary = ":tool",
    args = ["serve"],
    env = {"TOOL_MODE": "check"},
    preopens = {"/data": ":input.txt"},
    expected_stdout = "ok",
)
```

## Native

```sh
bazel build //pkg:tool
```

On a libstdc++ platform, such as `@rules_dx//qt/platforms:linux_x86_64`,
crates and their dependencies build without the static C++ runtime.
Binaries and tests link LLVM libunwind. The private `<name>_upstream`
targets are incompatible there; build `<name>`.

## Target triples

Your `MODULE.bazel` needs the Wasm triples for browser and WASI builds.
`dx init --apply --rust-target-triple=wasm32-unknown-unknown,wasm32-wasip1`
writes them into `MODULE.bazel.snippet`:

```python
rust = use_extension("@rules_rust//rust:extensions.bzl", "rust")
rust.toolchain(
    edition = "2021",
    extra_target_triples = ["wasm32-unknown-unknown", "wasm32-wasip1"],
    rustfmt_version = "1.98.1",
    versions = ["1.98.1"],
)
use_repo(rust, "rust_toolchains")
```

List only the triples you build.

## Browser

`rust_wasm_bindgen` builds the binary for `wasm32-unknown-unknown` and
emits the JavaScript bindings. Guard browser-only code with
`#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]`.

`rust_wasm_bindgen_test` runs the `#[wasm_bindgen_test]` cases of a
`rust_wasm_bindgen` bundle in a browser that Bazel fetches. It runs on
linux x86_64 with Firefox, where the host needs GTK 3, and only runs
locally. Other platforms skip it as incompatible. On linux x86_64 the
wrapper overrides `@rules_rust_wasm_bindgen//settings:test_browser`. Wasm
builds and browser tests run without coverage instrumentation, so
`bazel coverage` runs them but reports no Wasm coverage. Only these wrappers
guarantee that until upstream rules_rust does.

## WASI

`rust_wasi_module` builds the binary for `wasm32-wasip1` and exposes the
`.wasm` file. Building it fetches no runtime. The same module also builds
directly:

```sh
bazel build --platforms=@rules_rust//rust/platform:wasip1 //pkg:tool
```

`rust_wasi_test` runs the module under Wasmtime 49.0.2. The guest gets
only the declared `args`, `env` entries, and `preopens`. Ambient
environment and undeclared files stay denied.
`args`, `env`, `preopens` guest paths, `expected_stdout`, and
`expected_stderr_substring` are literal, so `$` needs no escaping. Text
containing `$(location ` or `$(execpath ` is not supported there.
`ambient` values get make-variable expansion.

| Attribute | Meaning |
| --- | --- |
| `binary` | The `rust_binary` to run. |
| `args` | Guest arguments. |
| `env` | Guest environment entries. |
| `preopens` | Guest directory mapped to one host file, staged alone. |
| `expected_stdout` | Exact guest stdout, trailing newlines trimmed. Default empty. |
| `expected_code` | Guest exit code. Default `0`. |
| `expected_stderr_substring` | Text guest stderr must contain. Default empty. |
| `runtime` | Executable that acts like `wasmtime run`. Default Wasmtime. |
| `ambient` | Host test environment entries. Never reach the guest. |

A custom `runtime` receives `DX_WASI_MANAGED_RUNTIME`, the path of the
default Wasmtime, so it can delegate. Proc macros and build scripts stay
on the execution platform. `rust_test` runs unit tests natively.
