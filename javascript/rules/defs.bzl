"""Experimental minimal JavaScript wrappers."""

load("@aspect_rules_jest//jest:defs.bzl", _jest_test = "jest_test")
load("@aspect_rules_js//js:defs.bzl", _js_binary = "js_binary", _js_library = "js_library", _js_test = "js_test")
load("@aspect_rules_js//js:providers.bzl", _JsInfo = "JsInfo", _js_info = "js_info")
load("@rules_rust_wasm_bindgen//:providers.bzl", _RustWasmBindgenInfo = "RustWasmBindgenInfo")
load("//libs/starlark:wrapper.bzl", "dx_binary_forward_kwargs", "dx_executable_forward_rule", "dx_forward_attrs", "dx_forwarded_optional", "dx_lcov_merger_attr", "dx_library_forward_rule", "dx_quality_sources", "dx_symlink_default_info", "dx_test_forward_kwargs", "dx_wrap")
load("//quality:sources.bzl", "QualitySourcesInfo")

_DX_JS_LIBRARY_PROVIDES = [
    _JsInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_DX_JS_TEST_PROVIDES = [
    DefaultInfo,
    QualitySourcesInfo,
]

_DX_JS_BINARY_PROVIDES = [
    DefaultInfo,
    QualitySourcesInfo,
]

_JS_EXTS = [".js", ".jsx", ".mjs", ".cjs"]

_DX_JS_SOURCE_SPECS = [("javascript", ["js", "mjs", "cjs"]), ("jsx", "jsx")]

_javascript_library_forward = dx_library_forward_rule(
    provides = _DX_JS_LIBRARY_PROVIDES,
    required_providers = [(_JsInfo, "JsInfo")],
    quality_specs = _DX_JS_SOURCE_SPECS,
    what = "javascript_*",
    allow_files = _JS_EXTS,
    upstream_providers = [[_JsInfo]],
)

_javascript_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = _DX_JS_BINARY_PROVIDES,
    required_providers = [],
    quality_specs = _DX_JS_SOURCE_SPECS,
    what = "javascript_*",
    allow_files = _JS_EXTS,
    upstream_providers = [[DefaultInfo]],
    optional_providers = [_JsInfo],
    runtime = "besteffort",
)

def _javascript_wrap_library(name, srcs, visibility = None, **kwargs):
    dx_wrap(name, _js_library, _javascript_library_forward, srcs, visibility = visibility, **kwargs)

def javascript_binary_upstream_data(srcs, data):
    """Computes the upstream data inputs for one javascript_binary."""
    return list(srcs or []) + list(data or [])

def _javascript_wrap_binary(name, srcs, visibility = None, **kwargs):
    upstream_kwargs = dict(kwargs)
    upstream_kwargs.pop("aspect_hints", None)
    if len(srcs) > 0:
        upstream_kwargs["data"] = javascript_binary_upstream_data(srcs, upstream_kwargs.get("data"))
    _js_binary(
        name = name + "_upstream",
        visibility = ["//visibility:private"],
        **upstream_kwargs
    )
    _javascript_binary_forward(
        name = name,
        upstream = name + "_upstream",
        srcs = srcs,
        visibility = visibility,
        **dx_binary_forward_kwargs(kwargs)
    )

def javascript_library(name, srcs, visibility = None, **kwargs):
    """Experimental minimal wrapper over js_library."""
    _javascript_wrap_library(name, srcs, visibility = visibility, **kwargs)

def _javascript_wasm_bindgen_library_impl(ctx):
    """Adapts one rust_wasm_bindgen target to JsInfo without rerunning bindgen."""
    bindgen = ctx.attr.bindgen
    if _RustWasmBindgenInfo not in bindgen:
        fail("javascript_wasm_bindgen_library requires bindgen to provide RustWasmBindgenInfo")
    info = bindgen[_RustWasmBindgenInfo]
    js = info.js.to_list()
    if len(js) == 0:
        fail("javascript_wasm_bindgen_library bindgen produces no JavaScript output")
    if info.wasm == None:
        fail("javascript_wasm_bindgen_library bindgen produces no Wasm output")
    ts = info.ts.to_list()
    if len(ts) == 0:
        fail("javascript_wasm_bindgen_library bindgen produces no TypeScript declarations")
    if info.snippets == None:
        fail("javascript_wasm_bindgen_library bindgen produces no snippets directory")
    dep_sources = [dep[_JsInfo].transitive_sources for dep in ctx.attr.deps]
    dep_types = [dep[_JsInfo].transitive_types for dep in ctx.attr.deps]
    dep_npm = [dep[_JsInfo].npm_sources for dep in ctx.attr.deps]
    dep_stores = [dep[_JsInfo].npm_package_store_infos for dep in ctx.attr.deps]
    js_info = _js_info(
        target = ctx.label,
        sources = info.js,
        types = info.ts,
        transitive_sources = depset(transitive = [info.js] + dep_sources),
        transitive_types = depset(transitive = [info.ts] + dep_types),
        npm_sources = depset(transitive = dep_npm),
        npm_package_store_infos = depset(transitive = dep_stores),
    )
    direct = [info.wasm, info.snippets] + js + ts
    transitive_runfiles = []
    if DefaultInfo in bindgen:
        transitive_runfiles.append(bindgen[DefaultInfo].default_runfiles.files)
    for dep in ctx.attr.deps:
        if DefaultInfo in dep:
            transitive_runfiles.append(dep[DefaultInfo].default_runfiles.files)
    return [
        DefaultInfo(
            files = depset(direct),
            runfiles = ctx.runfiles(files = direct, transitive_files = depset(transitive = transitive_runfiles)),
        ),
        js_info,
        info,
    ]

_javascript_wasm_bindgen_library = rule(
    implementation = _javascript_wasm_bindgen_library_impl,
    provides = [_JsInfo, DefaultInfo, _RustWasmBindgenInfo],
    attrs = {
        "bindgen": attr.label(mandatory = True, allow_files = True),
        "deps": attr.label_list(providers = [[_JsInfo]]),
    },
)

def javascript_wasm_bindgen_library(name, bindgen, deps = None, visibility = None, **kwargs):
    """Adapts one rust_wasm_bindgen target to JsInfo without rerunning bindgen."""
    _javascript_wasm_bindgen_library(
        name = name,
        bindgen = bindgen,
        deps = deps or [],
        visibility = visibility,
        **kwargs
    )

def javascript_binary(name, srcs = None, visibility = None, **kwargs):
    """Experimental minimal wrapper over js_binary."""
    effective_srcs = srcs if srcs != None else []
    _javascript_wrap_binary(name, effective_srcs, visibility = visibility, **kwargs)

def _javascript_test_forward_impl(ctx):
    upstream = ctx.attr.upstream
    env_inherit = list(ctx.attr.env_inherit) if ctx.attr.env_inherit else []
    if "TESTBRIDGE_TEST_ONLY" not in env_inherit:
        env_inherit.append("TESTBRIDGE_TEST_ONLY")
    out = [
        dx_symlink_default_info(ctx, "javascript_*"),
        testing.TestEnvironment({}, env_inherit),
        dx_quality_sources(ctx.files.srcs, _DX_JS_SOURCE_SPECS, str(ctx.label)),
    ]
    return out + dx_forwarded_optional(upstream, [InstrumentedFilesInfo, OutputGroupInfo], "javascript_*")

_javascript_test = rule(
    implementation = _javascript_test_forward_impl,
    test = True,
    provides = _DX_JS_TEST_PROVIDES,
    attrs = dx_forward_attrs(
        allow_files = _JS_EXTS,
        upstream_providers = [[DefaultInfo]],
        extra_attrs = {
            "env_inherit": attr.string_list(),
        } | dx_lcov_merger_attr(),
    ),
)

def javascript_test_rejection(kwargs):
    """Returns the rejection for forbidden javascript_test kwargs, or None."""
    if kwargs.get("auto_configure_reporters", True) == False:
        return ("javascript_test always uses jest with the standard " +
                "auto-configured reporters (Bazel test logs); " +
                "`auto_configure_reporters = False` is not supported. " +
                "Use javascript_js_test for a custom runner.")
    return None

_JAVASCRIPT_JEST_ONLY_KEYS = [
    "node_modules",
    "config",
    "snapshots",
    "run_in_band",
    "colors",
    "auto_configure_reporters",
    "auto_configure_test_sequencer",
    "snapshots_ext",
    "quiet_snapshot_updates",
]

def javascript_js_test_rejection(kwargs):
    """Returns the rejection for forbidden javascript_js_test kwargs, or None."""
    for key in _JAVASCRIPT_JEST_ONLY_KEYS:
        if key in kwargs:
            return ("javascript_js_test runs plain Node via js_test without " +
                    "Jest reporting; `" + key + "` is not supported. " +
                    "Use javascript_test for Jest behavior.")
    return None

def javascript_test_env(env_inherit):
    """Computes the effective test-runtime inherited environment."""
    env = list(env_inherit) if env_inherit != None else []
    if "TESTBRIDGE_TEST_ONLY" not in env:
        env.append("TESTBRIDGE_TEST_ONLY")
    return env

def javascript_test(name, srcs, node_modules, data = None, visibility = None, tags = None, env_inherit = None, **kwargs):
    """Experimental minimal wrapper over jest_test."""
    upstream_data = list(srcs) + (list(data) if data != None else [])
    effective_env = javascript_test_env(env_inherit)
    rejection = javascript_test_rejection(kwargs)
    if rejection != None:
        fail(rejection)
    upstream_kwargs = dict(kwargs)
    upstream_kwargs.pop("aspect_hints", None)
    if tags != None:
        upstream_tags = list(tags)
        if "manual" not in upstream_tags:
            upstream_tags.append("manual")
        upstream_kwargs["tags"] = upstream_tags
    elif upstream_kwargs.get("tags", None) == None:
        upstream_kwargs["tags"] = ["manual"]
    elif "manual" not in upstream_kwargs["tags"]:
        upstream_kwargs["tags"] = upstream_kwargs["tags"] + ["manual"]
    if "//:package_json" not in upstream_data:
        upstream_data.append("//:package_json")
    _jest_test(
        name = name + "_upstream",
        node_modules = node_modules,
        data = upstream_data,
        env_inherit = effective_env,
        visibility = ["//visibility:private"],
        **upstream_kwargs
    )
    forward_kwargs = dx_test_forward_kwargs(kwargs)
    _javascript_test(
        name = name,
        upstream = name + "_upstream",
        srcs = srcs,
        env_inherit = effective_env,
        visibility = visibility,
        tags = list(tags) if tags != None else forward_kwargs.pop("tags", None),
        **forward_kwargs
    )

def javascript_js_test(name, srcs, entry_point, data = None, visibility = None, tags = None, env_inherit = None, **kwargs):
    """Experimental minimal wrapper over js_test."""
    rejection = javascript_js_test_rejection(kwargs)
    if rejection != None:
        fail(rejection)
    upstream_data = list(srcs) + (list(data) if data != None else [])
    effective_env = javascript_test_env(env_inherit)
    upstream_kwargs = dict(kwargs)
    upstream_kwargs.pop("aspect_hints", None)
    if tags != None:
        upstream_tags = list(tags)
        if "manual" not in upstream_tags:
            upstream_tags.append("manual")
        upstream_kwargs["tags"] = upstream_tags
    elif upstream_kwargs.get("tags", None) == None:
        upstream_kwargs["tags"] = ["manual"]
    elif "manual" not in upstream_kwargs["tags"]:
        upstream_kwargs["tags"] = upstream_kwargs["tags"] + ["manual"]
    if "//:package_json" not in upstream_data:
        upstream_data.append("//:package_json")
    _js_test(
        name = name + "_upstream",
        entry_point = entry_point,
        data = upstream_data,
        env_inherit = effective_env,
        visibility = ["//visibility:private"],
        **upstream_kwargs
    )
    forward_kwargs = dx_test_forward_kwargs(kwargs)
    _javascript_test(
        name = name,
        upstream = name + "_upstream",
        srcs = srcs,
        env_inherit = effective_env,
        visibility = visibility,
        tags = list(tags) if tags != None else forward_kwargs.pop("tags", None),
        **forward_kwargs
    )
