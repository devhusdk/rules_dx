"""Minimal Rust wrappers."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_rust//rust:defs.bzl", _rust_binary = "rust_binary", _rust_clippy_test = "rust_clippy_test", _rust_common = "rust_common", _rust_library = "rust_library", _rust_proc_macro = "rust_proc_macro", _rust_shared_library = "rust_shared_library", _rust_static_library = "rust_static_library", _rust_test = "rust_test", _rustfmt_test = "rustfmt_test")
load("@rules_rust_wasm_bindgen//:defs.bzl", _rust_wasm_bindgen = "rust_wasm_bindgen", _rust_wasm_bindgen_test = "rust_wasm_bindgen_test", _rust_wasm_bindgen_toolchain = "rust_wasm_bindgen_toolchain")
load("//libs/starlark:wrapper.bzl", "dx_executable_forward_rule", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_wrap", "dx_wrap_test")
load("//quality:sources.bzl", "QualitySourcesInfo", "RUST")
load(":edition.bzl", "RUST_EDITION")

_DX_FORWARD_PROVIDES = [
    _rust_common.crate_info,
    _rust_common.dep_info,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_RUST_SOURCE_SPECS = [(RUST, "rs")]
_DX_RUST_SOURCE_EXTS = [".rs"]
_DX_RUST_CRATE_PROVIDERS = [(_rust_common.crate_info, "CrateInfo"), (_rust_common.dep_info, "DepInfo")]

_rust_forward = dx_library_forward_rule(
    provides = _DX_FORWARD_PROVIDES,
    required_providers = _DX_RUST_CRATE_PROVIDERS,
    quality_specs = _DX_RUST_SOURCE_SPECS,
    what = "rust_*",
    allow_files = _DX_RUST_SOURCE_EXTS,
    upstream_providers = [
        [_rust_common.crate_info],
        [_rust_common.test_crate_info],
    ],
)

_DX_CC_FORWARD_PROVIDES = [
    _rust_common.test_crate_info,
    _rust_common.dep_info,
    CcInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_rust_forward_cc = dx_library_forward_rule(
    provides = _DX_CC_FORWARD_PROVIDES,
    required_providers = [(_rust_common.test_crate_info, "TestCrateInfo"), (_rust_common.dep_info, "DepInfo"), (CcInfo, "CcInfo")],
    quality_specs = _DX_RUST_SOURCE_SPECS,
    what = "rust_*",
    allow_files = _DX_RUST_SOURCE_EXTS,
    upstream_providers = [[_rust_common.test_crate_info]],
)

_rust_forward_binary = dx_executable_forward_rule(
    kind = "executable",
    provides = _DX_FORWARD_PROVIDES,
    required_providers = _DX_RUST_CRATE_PROVIDERS,
    quality_specs = _DX_RUST_SOURCE_SPECS,
    what = "rust_*",
    allow_files = _DX_RUST_SOURCE_EXTS,
    upstream_providers = [
        [_rust_common.crate_info],
        [_rust_common.test_crate_info],
    ],
)

_rust_forward_test = dx_executable_forward_rule(
    kind = "test",
    provides = _DX_FORWARD_PROVIDES,
    required_providers = _DX_RUST_CRATE_PROVIDERS,
    quality_specs = _DX_RUST_SOURCE_SPECS,
    what = "rust_*",
    allow_files = _DX_RUST_SOURCE_EXTS,
    upstream_providers = [
        [_rust_common.crate_info],
        [_rust_common.test_crate_info],
    ],
    extra_attrs = dx_lcov_merger_attr(),
)

def rust_library(
        name,
        srcs,
        crate_name = None,
        edition = RUST_EDITION,
        visibility = None,
        **kwargs):
    """Minimal wrapper over rust_library."""
    dx_wrap(
        name,
        _rust_library,
        _rust_forward,
        srcs,
        crate_name = crate_name,
        edition = edition,
        visibility = visibility,
        **kwargs
    )

def rust_binary(
        name,
        srcs,
        crate_name = None,
        edition = RUST_EDITION,
        visibility = None,
        **kwargs):
    """Minimal wrapper over rust_binary."""
    dx_wrap(
        name,
        _rust_binary,
        _rust_forward_binary,
        srcs,
        crate_name = crate_name,
        edition = edition,
        visibility = visibility,
        **kwargs
    )

def rust_test(
        name,
        srcs = None,
        crate = None,
        edition = RUST_EDITION,
        visibility = None,
        **kwargs):
    """Minimal wrapper over rust_test."""
    upstream_kwargs = dict(kwargs)
    upstream_kwargs["crate"] = crate
    upstream_kwargs["edition"] = edition
    upstream_kwargs.setdefault("crate_name", name)
    dx_wrap_test(name, _rust_test, _rust_forward_test, srcs, visibility = visibility, upstream_kwargs = upstream_kwargs, **kwargs)

def rust_proc_macro(
        name,
        srcs,
        crate_name = None,
        edition = RUST_EDITION,
        visibility = None,
        **kwargs):
    """Minimal wrapper over rust_proc_macro."""
    dx_wrap(
        name,
        _rust_proc_macro,
        _rust_forward,
        srcs,
        crate_name = crate_name,
        edition = edition,
        visibility = visibility,
        **kwargs
    )

def rust_shared_library(
        name,
        srcs,
        crate_name = None,
        edition = RUST_EDITION,
        visibility = None,
        **kwargs):
    """Minimal wrapper over rust_shared_library."""
    dx_wrap(
        name,
        _rust_shared_library,
        _rust_forward_cc,
        srcs,
        crate_name = crate_name,
        edition = edition,
        visibility = visibility,
        **kwargs
    )

def rust_static_library(
        name,
        srcs,
        crate_name = None,
        edition = RUST_EDITION,
        visibility = None,
        **kwargs):
    """Minimal wrapper over rust_static_library."""
    dx_wrap(
        name,
        _rust_static_library,
        _rust_forward_cc,
        srcs,
        crate_name = crate_name,
        edition = edition,
        visibility = visibility,
        **kwargs
    )

def rustfmt_test(name, targets, size = "small", **kwargs):
    """Thin wrapper over upstream rustfmt_test."""
    _rustfmt_test(
        name = name,
        targets = targets,
        size = size,
        **kwargs
    )

def rust_clippy_test(name, targets, size = "small", **kwargs):
    """Thin wrapper over upstream rust_clippy_test."""
    _rust_clippy_test(
        name = name,
        targets = targets,
        size = size,
        **kwargs
    )

def rust_wasm_bindgen(name, wasm_file, **kwargs):
    """Thin wrapper over upstream rust_wasm_bindgen."""
    _rust_wasm_bindgen(
        name = name,
        wasm_file = wasm_file,
        **kwargs
    )

def rust_wasm_bindgen_test(name, wasm = None, **kwargs):
    """Thin wrapper over upstream rust_wasm_bindgen_test."""
    _rust_wasm_bindgen_test(
        name = name,
        wasm = wasm,
        **kwargs
    )

def rust_wasm_bindgen_toolchain(name, **kwargs):
    """Thin wrapper over upstream rust_wasm_bindgen_toolchain."""
    _rust_wasm_bindgen_toolchain(
        name = name,
        **kwargs
    )

_WASIP1_PLATFORM = Label("@rules_rust//rust/platform:wasip1")

_WASMTIME = Label("//wasi/runtime:wasmtime")

_WASI_DRIVER = Label("//wasi/testing:wasi_run_test.rs")

_WASI_DRIVER_DEP = Label("//wasi/testing:wasi_run")

def _wasip1_transition_impl(_settings, _attr):
    """Targets WASI preview 1 on one dependency edge."""
    return {"//command_line_option:platforms": [str(_WASIP1_PLATFORM)]}

_wasip1_transition = transition(
    implementation = _wasip1_transition_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms"],
)

def _rust_wasi_module_impl(ctx):
    """Forwards the one .wasm file the binary builds for WASI preview 1."""
    binary = ctx.attr.binary[0] if type(ctx.attr.binary) == "list" else ctx.attr.binary
    modules = [file for file in binary[DefaultInfo].files.to_list() if file.extension == "wasm"]
    if len(modules) != 1:
        fail("rust_wasi_module: '" + str(binary.label) + "' built " + str(len(modules)) +
             " .wasm files for wasm32-wasip1, want exactly one")
    return [DefaultInfo(files = depset(modules), runfiles = ctx.runfiles(files = modules))]

rust_wasi_module = rule(
    implementation = _rust_wasi_module_impl,
    doc = "Builds one rust_binary as a WASI preview 1 module.",
    attrs = {
        "binary": attr.label(
            doc = "The rust_binary to build for wasm32-wasip1.",
            mandatory = True,
            cfg = _wasip1_transition,
        ),
    },
)

def rust_wasi_test(
        name,
        binary,
        args = [],
        env = {},
        preopens = {},
        ambient = {},
        data = [],
        runtime = None,
        expected_stdout = "",
        expected_code = 0,
        size = "small",
        timeout = "short",
        visibility = None,
        **kwargs):
    """Runs one rust_binary as a WASI module with only the declared capabilities."""
    module = name + "_wasm"
    rust_wasi_module(
        name = module,
        binary = binary,
        testonly = True,
        visibility = ["//visibility:private"],
    )
    runner = runtime if runtime != None else _WASMTIME
    test_data = [":" + module, _WASMTIME] + ([runtime] if runtime != None else []) + data + sorted(preopens.values())
    test_env = dict(ambient)
    test_env.update({
        "DX_WASI_ARGS": json.encode(args),
        "DX_WASI_ENV": json.encode(env),
        "DX_WASI_EXPECTED": expected_stdout,
        "DX_WASI_EXPECTED_CODE": str(expected_code),
        "DX_WASI_MANAGED_RUNTIME": "$(rootpath %s)" % _WASMTIME,
        "DX_WASI_MODULE": "$(rootpath :%s)" % module,
        "DX_WASI_PREOPENS": json.encode({guest: "$(rootpath %s)" % host for guest, host in preopens.items()}),
        "DX_WASI_RUNTIME": "$(rootpath %s)" % runner,
    })
    rust_test(
        name = name,
        srcs = [_WASI_DRIVER],
        data = test_data,
        deps = [_WASI_DRIVER_DEP],
        env = test_env,
        size = size,
        timeout = timeout,
        visibility = visibility,
        **kwargs
    )
