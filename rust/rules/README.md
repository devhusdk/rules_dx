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

## Browser

`rust_wasm_bindgen` builds the binary for `wasm32-unknown-unknown` and
emits the JavaScript bindings. Guard browser-only code with
`#[cfg(all(target_arch = "wasm32", target_os = "unknown"))]`.

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

| Attribute | Meaning |
| --- | --- |
| `binary` | The `rust_binary` to run. |
| `args` | Guest arguments. |
| `env` | Guest environment entries. |
| `preopens` | Guest directory mapped to one host file, staged alone. |
| `expected_stdout` | Exact guest stdout, trailing newlines trimmed. Default empty. |
| `expected_code` | Guest exit code. Default `0`. |
| `runtime` | Executable that acts like `wasmtime run`. Default Wasmtime. |
| `ambient` | Host test environment entries. Never reach the guest. |

A custom `runtime` receives `DX_WASI_MANAGED_RUNTIME`, the path of the
default Wasmtime, so it can delegate. Proc macros and build scripts stay
on the execution platform. `rust_test` runs unit tests natively.
