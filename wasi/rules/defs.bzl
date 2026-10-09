"""Experimental minimal WASI wrappers."""

load("//rust/rules:defs.bzl", "rust_binary", "rust_test")

_WASIP1_PLATFORM = "@rules_rust//rust/platform:wasip1"

_WASI_TRIPLE = "wasm32-wasip1"

_MANAGED_RUNTIME = "//wasi/runtime:wasmtime"

_DRIVER = "//wasi/testing:wasi_run_test.rs"

_DRIVER_DEP = "//tools/testing:dx_testing"

def _wasi_transition_impl(_settings, _attr):
    """Selects the WASI preview 1 platform for one dependency edge."""
    return {"//command_line_option:platforms": [_WASIP1_PLATFORM]}

wasi_transition = transition(
    implementation = _wasi_transition_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms"],
)

def _wasi_module_impl(ctx):
    """Forwards the single WASI module built under the transition."""
    module = ctx.attr.module[0] if type(ctx.attr.module) == "list" else ctx.attr.module
    modules = [file for file in module[DefaultInfo].files.to_list() if file.extension == "wasm"]
    if len(modules) != 1:
        fail("wasi_binary: '" + str(module.label) + "' built " + str(len(modules)) +
             " .wasm files under " + _WASI_TRIPLE + ", want exactly one")
    return [DefaultInfo(files = depset(modules))]

_wasi_module_proxy = rule(
    implementation = _wasi_module_impl,
    attrs = {"module": attr.label(mandatory = True, cfg = wasi_transition)},
)

def wasi_binary(name, srcs, crate_name = None, edition = "2021", visibility = None, **kwargs):
    """Builds one Rust binary as a WASI preview 1 module."""
    rust_binary(
        name = name + "_module",
        srcs = srcs,
        crate_name = crate_name,
        edition = edition,
        visibility = ["//visibility:private"],
        **kwargs
    )
    _wasi_module_proxy(
        name = name,
        module = ":" + name + "_module",
        visibility = visibility,
    )

def _preopen_env(preopens):
    """Renders the guest-to-host preopen mapping for the test driver."""
    return {guest: "$(rootpath %s)" % label for guest, label in preopens.items()}

def wasi_test(
        name,
        module,
        args = [],
        env = {},
        ambient = {},
        preopens = {},
        data = [],
        runtime = _MANAGED_RUNTIME,
        expected_stdout = "",
        expected_code = 0,
        size = "small",
        timeout = "short",
        visibility = None,
        **kwargs):
    """Runs one WASI module under a declared runtime with declared capabilities."""
    preopen_labels = sorted(preopens.values())
    test_data = [module, runtime] + data + preopen_labels
    if runtime != _MANAGED_RUNTIME:
        test_data.append(_MANAGED_RUNTIME)
    test_env = dict(ambient)
    test_env.update({
        "DX_WASI_MODULE": "$(rootpath %s)" % module,
        "DX_WASI_RUNTIME": "$(rootpath %s)" % runtime,
        "DX_WASI_MANAGED_RUNTIME": "$(rootpath %s)" % _MANAGED_RUNTIME,
        "DX_WASI_ARGS": json.encode(args),
        "DX_WASI_ENV": json.encode(env),
        "DX_WASI_PREOPENS": json.encode(_preopen_env(preopens)),
        "DX_WASI_EXPECTED": expected_stdout,
        "DX_WASI_EXPECTED_CODE": str(expected_code),
    })
    rust_test(
        name = name,
        srcs = [_DRIVER],
        data = test_data,
        deps = [_DRIVER_DEP],
        env = test_env,
        size = size,
        timeout = timeout,
        visibility = visibility,
        **kwargs
    )
