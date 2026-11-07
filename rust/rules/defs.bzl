"""Minimal Rust wrappers."""

load("@bazel_skylib//rules:native_binary.bzl", "native_test")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_rust//rust:defs.bzl", _rust_binary = "rust_binary", _rust_common = "rust_common", _rust_library = "rust_library", _rust_proc_macro = "rust_proc_macro", _rust_shared_library = "rust_shared_library", _rust_static_library = "rust_static_library", _rust_test = "rust_test")
load("@rules_rust//rust/private:providers.bzl", "LintsInfo")
load("@rules_rust_wasm_bindgen//:defs.bzl", "RustWasmBindgenInfo", _rust_wasm_bindgen = "rust_wasm_bindgen", _rust_wasm_bindgen_test = "rust_wasm_bindgen_test")
load("@rules_shell//shell:sh_binary.bzl", "sh_binary")
load("//libs/starlark:resources.bzl", "dx_test_resource_tags")
load("//libs/starlark:wrapper.bzl", "dx_data_coverage_aspect", "dx_executable_forward_rule", "dx_forwarded_test_kwargs", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_native_coverage_attrs", "dx_native_coverage_toolchains", "dx_native_test_coverage", "dx_quality_sources", "dx_symlink_default_info", "dx_symlink_executable", "dx_upstream", "dx_wrap", "dx_wrap_binary", "dx_wrap_forward_kwargs", "dx_wrap_test", "dx_wrap_upstream_kwargs")
load("//quality:pipeline.bzl", "upstream_tags")
load("//quality:sources.bzl", "QualitySourcesInfo", "RUST")
load(":edition.bzl", "RUST_EDITION")
load(":msvc_crt.bzl", "MSVC_CRT_FROM_RUST", "dx_msvc_crt_aspect", "dx_msvc_crt_validate", "msvc_crt_static_settings")
load(":pdb.bzl", "REPRODUCIBLE_PDB_LINK_FLAGS")

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
_MSVC = Label("//rust/rules:msvc")
MSVC_LINK_FLAGS = ["-Clink-arg=kernel32.lib", "-Clink-arg=ntdll.lib"]
_MSVC_OPT = Label("//rust/rules:msvc_opt")

_MSVC_LINK = Label("//rust/tools/msvc_paths:msvc_link")

# rustc always passes /DEBUG and msvc_pdb_output.patch declares the PDB as an output;
# /Brepro makes the PDB GUID a content hash and msvc_link makes its paths relative (#8409, #8410).
MSVC_OPT_LINK_FLAGS = [
    "-Clinker=$(execpath {})".format(_MSVC_LINK),
    "-Clinker-flavor=lld-link",
    "-Clink-arg=/Brepro",
] + REPRODUCIBLE_PDB_LINK_FLAGS
RUST_FUZZ_LIBFUZZER_SYS = Label("@crates//:libfuzzer-sys")
_NO_STATIC_CPP_RUNTIMES = "-static_link_cpp_runtimes"
_EXTRA_RUSTC_FLAG = str(Label("@rules_rust//rust/settings:extra_rustc_flag"))
_CRT_STATIC_FLAG = "-Ctarget-feature=+crt-static"

_COVERAGE = "//command_line_option:collect_code_coverage"

_WASIP1_PLATFORM = Label("@rules_rust//rust/platform:wasip1")

_WASM32_PLATFORM = Label("@rules_rust//rust/platform:wasm32")

_PLATFORMS = "//command_line_option:platforms"

def _cpp_runtime_transition_impl(settings, attr):
    """Turns off static C++ runtime linking on libstdc++ platforms."""
    features = settings["//command_line_option:features"]
    from_rust = settings[MSVC_CRT_FROM_RUST]
    if attr.libstdcxx and _NO_STATIC_CPP_RUNTIMES not in features:
        features = features + [_NO_STATIC_CPP_RUNTIMES]
    if attr.msvc and (attr.crt_static or _CRT_STATIC_FLAG in settings[_EXTRA_RUSTC_FLAG]):
        return msvc_crt_static_settings(features)
    return {"//command_line_option:features": features, MSVC_CRT_FROM_RUST: from_rust}

_cpp_runtime_transition = transition(
    implementation = _cpp_runtime_transition_impl,
    inputs = ["//command_line_option:features", _EXTRA_RUSTC_FLAG, MSVC_CRT_FROM_RUST],
    outputs = ["//command_line_option:features", MSVC_CRT_FROM_RUST],
)

def _rust_binary_transition_impl(settings, attr):
    """Turns off static C++ runtime linking, and targets WASI when asked."""
    out = _cpp_runtime_transition_impl(settings, attr)
    out[_PLATFORMS] = settings[_PLATFORMS]
    out[_COVERAGE] = settings[_COVERAGE]
    if attr.wasm == "wasi":
        out[_PLATFORMS] = [str(_WASIP1_PLATFORM)]
        out[_COVERAGE] = False
    return out

_rust_binary_transition = transition(
    implementation = _rust_binary_transition_impl,
    inputs = ["//command_line_option:features", _EXTRA_RUSTC_FLAG, MSVC_CRT_FROM_RUST, _PLATFORMS, _COVERAGE],
    outputs = ["//command_line_option:features", MSVC_CRT_FROM_RUST, _PLATFORMS, _COVERAGE],
)

_LIBSTDCXX_ATTRS = {
    "libstdcxx": attr.bool(doc = "Whether the target platform uses libstdc++."),
    "msvc": attr.bool(doc = "Whether the target uses the MSVC ABI (cl.exe or clang-cl)."),
    "crt_static": attr.bool(doc = "Whether the target's own rustc_flags set +crt-static."),
}

def _libstdcxx_forward(rustc_flags = None):
    """Returns the forwarder kwargs that mark libstdc++ platforms and static CRT targets."""
    crt_static = type(rustc_flags) == "list" and any(["+crt-static" in flag for flag in rustc_flags])
    return {
        "libstdcxx": select({_LIBSTDCXX: True, "//conditions:default": False}),
        "msvc": select({_MSVC: True, "//conditions:default": False}),
        "crt_static": crt_static,
    }

def _libstdcxx_upstream(kwargs, link):
    """Returns upstream kwargs that build only below the forwarder on libstdc++ platforms."""
    out = dict(kwargs)
    out["target_compatible_with"] = (out.get("target_compatible_with") or []) + select({
        _LIBSTDCXX_READY: [],
        _LIBSTDCXX: [_INCOMPATIBLE],
        "//conditions:default": [],
    })
    if link:
        out["rustc_flags"] = (out.get("rustc_flags") or []) + select({
            _MSVC: MSVC_LINK_FLAGS,
            "//conditions:default": [],
        }) + select({
            _MSVC_OPT: MSVC_OPT_LINK_FLAGS,
            "//conditions:default": [],
        })
        out["compile_data"] = (out.get("compile_data") or []) + select({
            _MSVC_OPT: [_MSVC_LINK],
            "//conditions:default": [],
        })
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
    quiet_providers = [LintsInfo],
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
    upstream_aspects = [dx_msvc_crt_aspect],
    validate = dx_msvc_crt_validate,
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
    extra_attrs = _LIBSTDCXX_ATTRS | {"wasm": attr.string(doc = "Wasm target, or empty.")},
    upstream_cfg = _rust_binary_transition,
    upstream_aspects = [dx_msvc_crt_aspect],
    validate = dx_msvc_crt_validate,
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
    extra_attrs = dx_lcov_merger_attr() | _LIBSTDCXX_ATTRS | dx_native_coverage_attrs() | {
        "crate": attr.label(
            doc = "The crate under test, so format checks reach its files.",
            providers = [[_rust_common.crate_info], [_rust_common.test_crate_info]],
        ),
        "coverage_lib": attr.label(
            doc = "The library whose sources the test compiles itself, so coverage counts them as production code.",
        ),
    },
    upstream_cfg = _cpp_runtime_transition,
    upstream_aspects = [dx_data_coverage_aspect, dx_msvc_crt_aspect],
    validate = dx_msvc_crt_validate,
    coverage_providers = dx_native_test_coverage,
    toolchains = dx_native_coverage_toolchains(),
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
        extra_forward_kwargs = _libstdcxx_forward(kwargs.get("rustc_flags")),
        **kwargs
    )

def rust_binary(
        name,
        srcs,
        crate_name = None,
        edition = RUST_EDITION,
        visibility = None,
        wasm = None,
        **kwargs):
    """Minimal wrapper over rust_binary."""
    if wasm not in (None, "wasi", "browser"):
        fail("rust_binary: wasm must be \"wasi\" or \"browser\", got " + repr(wasm))
    if wasm == "browser":
        _rust_browser_binary(name, srcs, crate_name, edition, visibility, **kwargs)
        return
    dx_wrap_binary(
        name,
        _rust_binary,
        _rust_forward_binary,
        srcs,
        visibility = visibility,
        upstream_kwargs = _libstdcxx_upstream(dict(kwargs, crate_name = _crate_name(name, crate_name), edition = edition), link = True),
        extra_forward_kwargs = _libstdcxx_forward(kwargs.get("rustc_flags")) | {"wasm": wasm or ""},
        **kwargs
    )

def _rust_browser_binary(name, srcs, crate_name, edition, visibility, **kwargs):
    """Builds the binary for the browser and bundles it with wasm-bindgen."""
    upstream = name + "_upstream"
    wasm_input = name + "_wasm_input"
    bindgen = name + "_bindgen"
    _rust_binary(
        name = upstream,
        **dx_wrap_upstream_kwargs(_libstdcxx_upstream(dict(kwargs, crate_name = _crate_name(name, crate_name), edition = edition), link = True), srcs)
    )
    private = {
        "tags": ["manual"],
        "testonly": kwargs.get("testonly"),
        "target_compatible_with": kwargs.get("target_compatible_with"),
        "visibility": ["//visibility:private"],
    }
    _rust_wasm_input(name = wasm_input, wasm_file = upstream, **private)
    _rust_wasm_bindgen(name = bindgen, wasm_file = wasm_input, target = "web", out_name = name, **private)
    _rust_browser_forward(
        name = name,
        srcs = srcs,
        upstream = bindgen,
        wasm_file = wasm_input,
        visibility = visibility,
        **dx_wrap_forward_kwargs(kwargs)
    )

def rust_test(
        name,
        srcs = None,
        crate = None,
        edition = RUST_EDITION,
        visibility = None,
        wasm = None,
        preopens = None,
        runtime = None,
        coverage_lib = None,
        **kwargs):
    """Minimal wrapper over rust_test."""
    if wasm not in (None, "wasi", "browser"):
        fail("rust_test: wasm must be \"wasi\" or \"browser\", got " + repr(wasm))
    if wasm != "wasi" and (preopens != None or runtime != None):
        fail("rust_test: preopens and runtime need wasm = \"wasi\"")
    if wasm == "browser":
        if (crate == None) == (srcs == None):
            fail("rust_test: wasm = \"browser\" needs srcs or crate, not both")
        if srcs != None:
            crate_root = kwargs.pop("crate_root", None)
            if crate_root == None and type(srcs) == "list" and len(srcs) == 1:
                crate_root = srcs[0]
            binary = {key: kwargs.pop(key) for key in ("deps", "proc_macro_deps", "crate_name", "compile_data", "rustc_env", "rustc_env_files", "aliases") if key in kwargs}
            binary |= {key: kwargs[key] for key in ("crate_features", "rustc_flags") if key in kwargs}
            if crate_root != None:
                binary["crate_root"] = crate_root
            rust_binary(
                name = name + "_crate",
                srcs = srcs,
                edition = edition,
                wasm = "browser",
                testonly = True,
                tags = ["manual"],
                visibility = ["//visibility:private"],
                target_compatible_with = kwargs.get("target_compatible_with"),
                **binary
            )
            crate = ":" + name + "_crate"
        _rust_browser_test(name, crate, visibility = visibility, **kwargs)
        return
    if wasm == "wasi":
        _rust_wasi_test(name, srcs, crate, edition, visibility, preopens, runtime, **kwargs)
        return
    upstream_kwargs = _libstdcxx_upstream(kwargs, link = True)
    upstream_kwargs["crate"] = crate
    upstream_kwargs["edition"] = edition
    upstream_kwargs["crate_name"] = _crate_name(name, kwargs.get("crate_name"))
    forward_kwargs = _libstdcxx_forward(kwargs.get("rustc_flags"))
    if crate != None:
        forward_kwargs["crate"] = crate
    if coverage_lib != None:
        forward_kwargs["coverage_lib"] = coverage_lib
    dx_wrap_test(name, _rust_test, _rust_forward_test, srcs, visibility = visibility, upstream_kwargs = upstream_kwargs, extra_forward_kwargs = forward_kwargs, **kwargs)

def rust_benchmark(
        name,
        srcs = None,
        crate = None,
        crate_features = [],
        tags = [],
        data = [],
        visibility = None,
        **kwargs):
    """A rust_test run by `dx bench`; it reports its bencher output as dx-bench.json."""
    binary = name + "_bin"
    rust_test(
        name = binary,
        srcs = srcs,
        crate = crate,
        crate_features = crate_features + ["dx_bench"],
        tags = ["manual"],
        visibility = ["//visibility:private"],
        **(dict(use_libtest_harness = False) | kwargs)
    )
    native_test(
        name = name,
        src = Label("//rust/rules/bench_report"),
        args = ["$(rootpath :%s)" % binary, "--bench", "--output-format", "bencher"],
        data = data + [":" + binary],
        tags = tags + ["dx-bench", "manual"],
        visibility = visibility,
    )

def _rust_wasi_test(name, srcs, crate, edition, visibility, preopens, runtime, **kwargs):
    """Builds the test crate for WASI preview 1 and runs it under Wasmtime."""
    upstream_kwargs = {key: value for key, value in kwargs.items() if key not in ("args", "env", "data")}
    upstream_kwargs["crate"] = crate
    upstream_kwargs["edition"] = edition
    upstream_kwargs["crate_name"] = _crate_name(name, kwargs.get("crate_name"))
    forward_kwargs = {
        "data": kwargs.get("data", []),
        "guest_env": kwargs.get("env", {}),
        "preopens": preopens or {},
    }
    if runtime != None:
        forward_kwargs["runtime"] = runtime
    dx_wrap_test(name, _rust_test, _rust_wasi_run_test, srcs, visibility = visibility, upstream_kwargs = upstream_kwargs, extra_forward_kwargs = forward_kwargs, coverage_collector = False, **kwargs)

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
        extra_forward_kwargs = _libstdcxx_forward(kwargs.get("rustc_flags")),
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
        extra_forward_kwargs = _libstdcxx_forward(kwargs.get("rustc_flags")),
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
        extra_forward_kwargs = _libstdcxx_forward(kwargs.get("rustc_flags")),
        **kwargs
    )

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
    for provider in (_rust_common.crate_info, _rust_common.dep_info):
        if provider in target:
            providers.append(target[provider])
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

def _wasm32_transition_impl(_settings, _attr):
    """Targets wasm32 without coverage on one dependency edge."""
    return {_PLATFORMS: [str(_WASM32_PLATFORM)], _COVERAGE: False}

_wasm32_transition = transition(
    implementation = _wasm32_transition_impl,
    inputs = [],
    outputs = [_PLATFORMS, _COVERAGE],
)

def _rust_browser_forward_impl(ctx):
    """Forwards the bindgen bundle with the crate the browser test reads."""
    upstream = ctx.attr.upstream
    crate = ctx.attr.wasm_file[0]
    providers = [
        upstream[DefaultInfo],
        upstream[RustWasmBindgenInfo],
        crate[_rust_common.crate_info],
        crate[_rust_common.dep_info],
        dx_quality_sources(ctx.files.srcs, _DX_RUST_SOURCE_SPECS, str(ctx.label)),
    ]
    if OutputGroupInfo in upstream:
        providers.append(upstream[OutputGroupInfo])
    return providers

_rust_browser_forward = rule(
    implementation = _rust_browser_forward_impl,
    doc = "Forwards one wasm-bindgen bundle of a rust_binary.",
    attrs = {
        "srcs": attr.label_list(doc = "The binary sources.", allow_files = _DX_RUST_SOURCE_EXTS),
        "upstream": attr.label(doc = "The wasm-bindgen bundle.", mandatory = True, providers = [RustWasmBindgenInfo]),
        "wasm_file": attr.label(doc = "The Wasm binary.", mandatory = True, cfg = _wasm32_transition),
    },
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
    crate = ctx.attr.crate
    providers.append(crate[_rust_common.crate_info])
    if _rust_common.dep_info in crate:
        providers.append(crate[_rust_common.dep_info])
    return providers

_rust_wasm_bindgen_browser_test = rule(
    implementation = _rust_wasm_bindgen_browser_test_impl,
    doc = "Forwards one upstream rust_wasm_bindgen_test built for the qualified browser.",
    test = True,
    attrs = {
        "browser": attr.string(doc = "The managed browser of this platform cell, or empty."),
        "crate": attr.label(
            doc = "The Rust crate whose providers format and lint checks read.",
            mandatory = True,
            providers = [[_rust_common.crate_info]],
        ),
        "data": attr.label_list(doc = "Runtime files for forwarded args.", allow_files = True),
        "upstream": attr.label(
            doc = "The upstream rust_wasm_bindgen_test.",
            mandatory = True,
            cfg = _browser_transition,
        ),
    },
)

_FORWARDER_ONLY_KWARGS = ("tags", "timeout", "shard_count", "size", "local", "flaky", "args")

def _rust_browser_test(name, wasm, **kwargs):
    """Runs the bundle under a managed, platform-qualified browser."""
    forward = dx_forwarded_test_kwargs(kwargs)
    forward.setdefault("size", "large")
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
        crate = wasm,
        upstream = name + "_upstream",
        browser = select(cells | {"//conditions:default": ""}),
        tags = dx_test_resource_tags(forward["size"], user_tags + ([] if _NO_REMOTE_EXEC in user_tags else [_NO_REMOTE_EXEC])),
        target_compatible_with = (kwargs.get("target_compatible_with") or []) + select(
            {cell: [] for cell in _BROWSER_CELLS.keys()} | {"//conditions:default": [_INCOMPATIBLE]},
        ),
        visibility = kwargs.get("visibility"),
        **forward
    )

_WASMTIME = Label("//wasi/runtime:wasmtime")

_EXTRA_EXEC = "//command_line_option:extra_execution_platforms"

_HOST_PLATFORM = Label("@platforms//host")

def _wasi_test_transition_impl(settings, _attr):
    """Targets WASI preview 1 without coverage on one dependency edge."""
    return {
        _PLATFORMS: [str(_WASIP1_PLATFORM)],
        _COVERAGE: False,
        _EXTRA_EXEC: settings[_EXTRA_EXEC] + [str(_HOST_PLATFORM), str(_WASIP1_PLATFORM)],
    }

_wasi_test_transition = transition(
    implementation = _wasi_test_transition_impl,
    inputs = [_EXTRA_EXEC],
    outputs = [_PLATFORMS, _COVERAGE, _EXTRA_EXEC],
)

def _rust_wasi_run_test_impl(ctx):
    """Runs the upstream test module through the shared Wasmtime runner."""
    upstream = dx_upstream(ctx)
    modules = [file for file in upstream[DefaultInfo].files.to_list() if file.extension == "wasm"]
    if len(modules) != 1:
        fail("rust_test: '" + str(upstream.label) + "' built " + str(len(modules)) +
             " .wasm files for wasm32-wasip1, want exactly one; set srcs or crate to a Rust crate that builds one test module")
    runner = ctx.attr._runner[0]
    link = dx_symlink_executable(ctx, runner[DefaultInfo].files_to_run.executable)
    preopen_files = {guest: target[DefaultInfo].files.to_list()[0] for guest, target in ctx.attr.preopens.items()}
    runfiles = ctx.runfiles(files = [link] + modules + [ctx.file.runtime, ctx.file._managed] + preopen_files.values() + ctx.files.data)
    for dep in [runner, ctx.attr.runtime, ctx.attr._managed] + ctx.attr.data + ctx.attr.preopens.values():
        runfiles = runfiles.merge(dep[DefaultInfo].default_runfiles)
    environment = {
        "DX_WASI_ENV": json.encode(ctx.attr.guest_env),
        "DX_WASI_MANAGED_RUNTIME": ctx.file._managed.short_path,
        "DX_WASI_MODULE": modules[0].short_path,
        "DX_WASI_PREOPENS": json.encode({guest: file.short_path for guest, file in preopen_files.items()}),
        "DX_WASI_RUNTIME": ctx.file.runtime.short_path,
    }
    return [
        DefaultInfo(executable = link, files = depset([link]), runfiles = runfiles),
        RunEnvironmentInfo(environment = environment),
    ] + [upstream[p] for p in (_rust_common.crate_info, _rust_common.test_crate_info, _rust_common.dep_info) if p in upstream]

_rust_wasi_run_test = rule(
    implementation = _rust_wasi_run_test_impl,
    doc = "Runs one Rust test crate as a WASI preview 1 module under Wasmtime.",
    test = True,
    attrs = {
        "data": attr.label_list(doc = "Files the test needs at run time.", allow_files = True),
        "guest_env": attr.string_dict(doc = "Environment variables the guest sees."),
        "preopens": attr.string_keyed_label_dict(doc = "Guest directory to the file staged there.", allow_files = True),
        "runtime": attr.label(doc = "The WASI runtime. On Windows it must be a native `.exe`, `.cmd` or `.bat`.", default = _WASMTIME, allow_single_file = True),
        "srcs": attr.label_list(doc = "The test crate sources.", allow_files = True),
        "upstream": attr.label(doc = "The test crate built for wasm32-wasip1.", mandatory = True, cfg = _wasi_test_transition),
        "_managed": attr.label(default = _WASMTIME, allow_single_file = True),
        "_runner": attr.label(default = "//wasi/testing:wasi_test_runner", executable = True, cfg = _no_coverage_transition),
    },
)

_FUZZ_RUSTC_FLAGS = [
    "-Cpasses=sancov-module",
    "-Cllvm-args=-sanitizer-coverage-level=4",
    "-Cllvm-args=-sanitizer-coverage-inline-8bit-counters",
    "-Cllvm-args=-sanitizer-coverage-pc-table",
    "-Zsanitizer=address",
    "-Zexternal-clangrt",
]

def _rust_fuzz_transition_impl(_settings, _attr):
    return {
        "@llvm//config:asan": True,
        "@llvm//config:fuzzer": True,
    }

_rust_fuzz_transition = transition(
    implementation = _rust_fuzz_transition_impl,
    inputs = [],
    outputs = ["@llvm//config:asan", "@llvm//config:fuzzer"],
)

def _rust_fuzz_binary_impl(ctx):
    binary = ctx.attr.binary
    executable = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.symlink(output = executable, target_file = binary[DefaultInfo].files_to_run.executable, is_executable = True)
    runfiles = ctx.runfiles(files = [executable]).merge(binary[DefaultInfo].default_runfiles)
    return [DefaultInfo(executable = executable, runfiles = runfiles)]

_rust_fuzz_binary = rule(
    implementation = _rust_fuzz_binary_impl,
    attrs = {
        "binary": attr.label(executable = True, cfg = "target", mandatory = True),
    },
    cfg = _rust_fuzz_transition,
    executable = True,
    doc = "Builds `binary` with the pinned LLVM's libFuzzer runtime.",
)

def rust_fuzz_test(name, srcs, deps = None, visibility = None, tags = None, rustc_flags = None, rustc_env = None, libfuzzer_sys = None, **kwargs):
    """A libFuzzer binary built with sancov and AddressSanitizer plus a `<name>_run` runner for `dx fuzz`.

    Needs a toolchain that accepts `-Z` flags; the build sets RUSTC_BOOTSTRAP=1 for it.
    `libfuzzer_sys` overrides the default libfuzzer-sys from rules_dx's hub.
    """
    rust_binary(
        name = name + "_binary",
        srcs = srcs,
        deps = (deps or []) + [libfuzzer_sys or RUST_FUZZ_LIBFUZZER_SYS],
        rustc_flags = _FUZZ_RUSTC_FLAGS + (rustc_flags or []),
        rustc_env = dict(rustc_env or {}, RUSTC_BOOTSTRAP = "1"),
        visibility = ["//visibility:private"],
        testonly = True,
        tags = ["manual"],
        **kwargs
    )
    _rust_fuzz_binary(
        name = name,
        binary = ":" + name + "_binary",
        visibility = visibility,
        testonly = True,
        tags = (tags or []) + ["dx-fuzz", "manual"],
    )
    sh_binary(
        name = name + "_run",
        srcs = [Label("//cc/rules:fuzz_run.sh")],
        args = ["$(rootpath :" + name + ")"],
        data = [":" + name],
        testonly = True,
        visibility = visibility,
    )
