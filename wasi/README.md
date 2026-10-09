# WASI

Build and run Rust modules under WASI preview 1 with a managed replaceable runtime.

```python
load("@rules_dx//wasi/rules:defs.bzl", "wasi_binary", "wasi_test")

wasi_binary(
    name = "tool",
    srcs = ["src/main.rs"],
    crate_name = "tool",
    crate_root = "src/main.rs",
)

wasi_test(
    name = "tool_run",
    module = ":tool",
    args = ["serve"],
    env = {"TOOL_MODE": "check"},
    preopens = {"/data": ":input.txt"},
    expected_stdout = "...",
)
```

`wasi_binary` compiles the crate for `wasm32-wasip1` and exposes the `.wasm` file.
`wasi_test` runs the module under Wasmtime 49.0.2 with only the declared arguments,
environment entries, and directory preopens. Ambient environment and undeclared
files stay denied. Point `runtime` at an executable acting like `wasmtime run` to
override the managed default; it receives `DX_WASI_MANAGED_RUNTIME` for delegation.
