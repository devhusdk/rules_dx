"""Minimal Rust wrappers."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_rust//rust:defs.bzl", _rust_binary = "rust_binary", _rust_clippy_test = "rust_clippy_test", _rust_common = "rust_common", _rust_library = "rust_library", _rust_proc_macro = "rust_proc_macro", _rust_shared_library = "rust_shared_library", _rust_static_library = "rust_static_library", _rust_test = "rust_test", _rustfmt_test = "rustfmt_test")
load("@rules_rust_wasm_bindgen//:defs.bzl", _rust_wasm_bindgen = "rust_wasm_bindgen", _rust_wasm_bindgen_test = "rust_wasm_bindgen_test", _rust_wasm_bindgen_toolchain = "rust_wasm_bindgen_toolchain")
load("//libs/starlark:wrapper.bzl", "dx_data_coverage_aspect", "dx_executable_forward_rule", "dx_forwarded_test_kwargs", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_native_coverage_attrs", "dx_native_test_coverage", "dx_symlink_default_info", "dx_upstream", "dx_wrap", "dx_wrap_binary", "dx_wrap_test")
load("//quality:pipeline.bzl", "upstream_tags")
load("//quality:sources.bzl", "QualitySourcesInfo", "RUST")
load(":edition.bzl", "RUST_EDITION")

_DX_FORWARD_PROVIDES = [
    _rust_common.crate_info,
    _rust_common.dep_info,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_LIBSTDCXX = Label("//rust/rules:libstdcxx")
_LIBSTDCXX_READY = Label("//rust/rules:libstdcxx_ready")
_INCOMPATIBLE = Label("@platforms//:incompatible")
_UNWIND = Label("//rust/rules:unwind")
_NO_STATIC_CPP_RUNTIMES = "-static_link_cpp_runtimes"

def _cpp_runtime_transition_impl(settings, attr):
    """Turns off static C++ runtime linking on libstdc++ platforms."""
    features = settings["//command_line_option:features"]
    if attr.libstdcxx and _NO_STATIC_CPP_RUNTIMES not in features:
        features = features + [_NO_STATIC_CPP_RUNTIMES]
    return {"//command_line_option:features": features}

_cpp_runtime_transition = transition(
    implementation = _cpp_runtime_transition_impl,
    inputs = ["//command_line_option:features"],
    outputs = ["//command_line_option:features"],
)

_LIBSTDCXX_ATTRS = {
    "libstdcxx": attr.bool(doc = "Whether the target platform uses libstdc++."),
}

def _libstdcxx_forward():
    """Returns the forwarder kwargs that mark libstdc++ platforms."""
    return {"libstdcxx": select({_LIBSTDCXX: True, "//conditions:default": False})}

def _libstdcxx_upstream(kwargs, link):
    """Returns upstream kwargs that build only below the forwarder on libstdc++ platforms."""
    out = dict(kwargs)
    out["target_compatible_with"] = (out.get("target_compatible_with") or []) + select({
        _LIBSTDCXX_READY: [],
        _LIBSTDCXX: [_INCOMPATIBLE],
        "//conditions:default": [],
    })
    if link:
        out["link_deps"] = (out.get("link_deps") or []) + select({
            _LIBSTDCXX: [_UNWIND],
            "//conditions:default": [],
        })
    return out

def _crate_name(name, crate_name):
    """Returns crate_name, or the target name with - and / replaced by _."""
    if crate_name != None:
        return crate_name
    return name.replace("-", "_").replace("/", "_")

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
    extra_attrs = _LIBSTDCXX_ATTRS,
    upstream_cfg = _cpp_runtime_transition,
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
    extra_attrs = _LIBSTDCXX_ATTRS,
    upstream_cfg = _cpp_runtime_transition,
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
    extra_attrs = _LIBSTDCXX_ATTRS,
    upstream_cfg = _cpp_runtime_transition,
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
    extra_attrs = dx_lcov_merger_attr() | _LIBSTDCXX_ATTRS | dx_native_coverage_attrs(),
    upstream_cfg = _cpp_runtime_transition,
    upstream_aspects = [dx_data_coverage_aspect],
    coverage_providers = dx_native_test_coverage,
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
        visibility = visibility,
        upstream_kwargs = _libstdcxx_upstream(dict(kwargs, crate_name = _crate_name(name, crate_name), edition = edition), link = False),
        extra_forward_kwargs = _libstdcxx_forward(),
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
    dx_wrap_binary(
        name,
        _rust_binary,
        _rust_forward_binary,
        srcs,
        visibility = visibility,
        upstream_kwargs = _libstdcxx_upstream(dict(kwargs, crate_name = _crate_name(name, crate_name), edition = edition), link = True),
        extra_forward_kwargs = _libstdcxx_forward(),
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
    upstream_kwargs = _libstdcxx_upstream(kwargs, link = True)
    upstream_kwargs["crate"] = crate
    upstream_kwargs["edition"] = edition
    upstream_kwargs["crate_name"] = _crate_name(name, kwargs.get("crate_name"))
    dx_wrap_test(name, _rust_test, _rust_forward_test, srcs, visibility = visibility, upstream_kwargs = upstream_kwargs, extra_forward_kwargs = _libstdcxx_forward(), **kwargs)

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
        visibility = visibility,
        upstream_kwargs = _libstdcxx_upstream(dict(kwargs, crate_name = _crate_name(name, crate_name), edition = edition), link = False),
        extra_forward_kwargs = _libstdcxx_forward(),
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
        visibility = visibility,
        upstream_kwargs = _libstdcxx_upstream(dict(kwargs, crate_name = _crate_name(name, crate_name), edition = edition), link = True),
        extra_forward_kwargs = _libstdcxx_forward(),
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
        visibility = visibility,
        upstream_kwargs = _libstdcxx_upstream(dict(kwargs, crate_name = _crate_name(name, crate_name), edition = edition), link = False),
        extra_forward_kwargs = _libstdcxx_forward(),
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

_COVERAGE = "//command_line_option:collect_code_coverage"

def _no_coverage_transition_impl(_settings, _attr):
    """Turns off coverage instrumentation on one dependency edge."""
    return {_COVERAGE: False}

_no_coverage_transition = transition(
    implementation = _no_coverage_transition_impl,
    inputs = [],
    outputs = [_COVERAGE],
)

def _rust_wasm_input_impl(ctx):
    """Forwards one Wasm module built without coverage instrumentation."""
    target = ctx.attr.wasm_file[0]
    providers = [DefaultInfo(
        files = target[DefaultInfo].files,
        runfiles = target[DefaultInfo].default_runfiles,
    )]
    if _rust_common.crate_info in target:
        providers.append(target[_rust_common.crate_info])
    return providers

_rust_wasm_input = rule(
    implementation = _rust_wasm_input_impl,
    doc = "Builds one Wasm module without coverage instrumentation.",
    attrs = {
        "wasm_file": attr.label(
            doc = "The Wasm module or Rust binary to forward.",
            mandatory = True,
            cfg = _no_coverage_transition,
        ),
    },
)

def rust_wasm_bindgen(name, wasm_file, **kwargs):
    """Thin wrapper over upstream rust_wasm_bindgen."""
    wasm_input = name + "_wasm_input"
    _rust_wasm_input(
        name = wasm_input,
        wasm_file = wasm_file,
        tags = ["manual"],
        testonly = kwargs.get("testonly"),
        target_compatible_with = kwargs.get("target_compatible_with"),
        visibility = ["//visibility:private"],
    )
    _rust_wasm_bindgen(
        name = name,
        wasm_file = wasm_input,
        **kwargs
    )

_NO_REMOTE_EXEC = "no-remote-exec"

_TEST_BROWSER = str(Label("@rules_rust_wasm_bindgen//settings:test_browser"))

# The only place browser-test qualification lives: one row per qualified
# platform cell, mapping it to the browser its managed toolchain runs there.
# Every other cell skips the browser test as incompatible.
_BROWSER_CELLS = {
    Label("//rust/rules:browser_linux_x86_64"): "firefox",
}

def _browser_transition_impl(settings, attr):
    """Pins the managed test browser for the qualified platform cell."""
    browser = attr.browser or settings[_TEST_BROWSER]
    return {_TEST_BROWSER: browser, _COVERAGE: False}

_browser_transition = transition(
    implementation = _browser_transition_impl,
    inputs = [_TEST_BROWSER],
    outputs = [_TEST_BROWSER, _COVERAGE],
)

def _rust_wasm_bindgen_browser_test_impl(ctx):
    """Runs the upstream browser test under the qualified cell's browser."""
    upstream = dx_upstream(ctx)
    providers = [dx_symlink_default_info(ctx, "rust_wasm_bindgen_test")]
    if RunEnvironmentInfo in upstream:
        providers.append(upstream[RunEnvironmentInfo])
    return providers

_rust_wasm_bindgen_browser_test = rule(
    implementation = _rust_wasm_bindgen_browser_test_impl,
    doc = "Forwards one upstream rust_wasm_bindgen_test built for the qualified browser.",
    test = True,
    attrs = {
        "browser": attr.string(doc = "The managed browser of this platform cell, or empty."),
        "data": attr.label_list(doc = "Runtime files for forwarded args.", allow_files = True),
        "upstream": attr.label(
            doc = "The upstream rust_wasm_bindgen_test.",
            mandatory = True,
            cfg = _browser_transition,
        ),
    },
)

_FORWARDER_ONLY_KWARGS = ("tags", "timeout", "shard_count", "size", "local", "flaky", "args")

def rust_wasm_bindgen_test(name, wasm = None, **kwargs):
    """Wraps upstream rust_wasm_bindgen_test with a managed, platform-qualified browser.

    The test runs only on qualified platform cells (today linux x86_64, with
    the Firefox and geckodriver that Bazel fetches; the host needs GTK 3).
    On those cells the wrapper sets
    `@rules_rust_wasm_bindgen//settings:test_browser` for the test, which
    overrides any value given on the command line. Every other cell reports
    the test as incompatible and skips it. The test runs locally only
    (`no-remote-exec`), since the browser needs a display-capable host.
    The whole test subgraph builds without coverage instrumentation, so
    `bazel coverage` runs the test but reports no Wasm coverage.
    """
    forward = dx_forwarded_test_kwargs(kwargs)
    upstream = {k: v for k, v in kwargs.items() if k not in _FORWARDER_ONLY_KWARGS}
    user_tags = forward.pop("tags", [])
    _rust_wasm_bindgen_test(
        name = name + "_upstream",
        wasm = wasm,
        tags = ["manual", _NO_REMOTE_EXEC] + [t for t in upstream_tags(user_tags) if t not in ("manual", _NO_REMOTE_EXEC)],
        visibility = ["//visibility:private"],
        **{k: v for k, v in upstream.items() if k != "visibility"}
    )
    cells = {cell: browser for cell, browser in _BROWSER_CELLS.items()}
    _rust_wasm_bindgen_browser_test(
        name = name,
        upstream = name + "_upstream",
        browser = select(cells | {"//conditions:default": ""}),
        tags = user_tags + ([] if _NO_REMOTE_EXEC in user_tags else [_NO_REMOTE_EXEC]),
        target_compatible_with = (kwargs.get("target_compatible_with") or []) + select(
            {cell: [] for cell in _BROWSER_CELLS.keys()} | {"//conditions:default": [_INCOMPATIBLE]},
        ),
        visibility = kwargs.get("visibility"),
        **forward
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

def _literal(value):
    """Keeps a guest value literal through make-variable expansion."""
    return value.replace("$", "$$")

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
        expected_stderr_substring = "",
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
        tags = kwargs.get("tags", []),
        target_compatible_with = kwargs.get("target_compatible_with", []),
    )
    runner = runtime if runtime != None else _WASMTIME
    test_data = []
    seen = {}
    for item in [":" + module, _WASMTIME] + ([runtime] if runtime != None else []) + data + sorted(preopens.values()):
        key = str(native.package_relative_label(item))
        if key not in seen:
            seen[key] = True
            test_data.append(item)
    test_env = dict(ambient)
    test_env.update({
        "DX_WASI_ARGS": _literal(json.encode(args)),
        "DX_WASI_ENV": _literal(json.encode(env)),
        "DX_WASI_EXPECTED": _literal(expected_stdout),
        "DX_WASI_EXPECTED_CODE": str(expected_code),
        "DX_WASI_EXPECTED_STDERR": _literal(expected_stderr_substring),
        "DX_WASI_MANAGED_RUNTIME": "$(rootpath %s)" % _WASMTIME,
        "DX_WASI_MODULE": "$(rootpath :%s)" % module,
        "DX_WASI_PREOPENS": json.encode({_literal(guest): "$(rootpath %s)" % host for guest, host in preopens.items()}),
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
