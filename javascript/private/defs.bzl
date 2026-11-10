"""Minimal JavaScript wrappers."""

load("@aspect_rules_jest//jest:defs.bzl", _jest_test = "jest_test")
load("@aspect_rules_js//js:defs.bzl", _js_binary = "js_binary", _js_library = "js_library", _js_test = "js_test")
load("@aspect_rules_js//js:providers.bzl", _JsInfo = "JsInfo", _js_info = "js_info")
load("@rules_rust_wasm_bindgen//:providers.bzl", _RustWasmBindgenInfo = "RustWasmBindgenInfo")
load("//core:entry.bzl", "dx_entry_quality_srcs")
load("//core:fuzz.bzl", "dx_fuzz_test")
load("//core:fuzz_bench.bzl", "merge_tags", "with_default_size")
load("//core:node_test.bzl", "node_jest_upstream_attrs", "node_test_forward_rule", "node_test_runner_rejection", "node_test_wrap_args")
load("//core:wrapper.bzl", "DX_EXEC_PROVIDES", "dx_binary_forward_kwargs", "dx_binary_upstream_kwargs", "dx_executable_forward_rule", "dx_library_forward_rule", "dx_wrap", "dx_wrap_test")
load("//core/quality:sources.bzl", "QualitySourcesInfo")

visibility(["//..."])

_DX_JS_LIBRARY_PROVIDES = [
    _JsInfo,
    DefaultInfo,
    InstrumentedFilesInfo,
    QualitySourcesInfo,
]

_JS_EXTS = [".js", ".jsx", ".mjs", ".cjs"]

_JS_LIBRARY_EXTS = _JS_EXTS + [".css", ".scss", ".less", ".html", ".graphql", ".gql", ".json5", ".jsonc"]

_DX_JS_SOURCE_SPECS = [("javascript", ["js", "mjs", "cjs"]), ("jsx", "jsx")]

_DX_JS_LIBRARY_SOURCE_SPECS = _DX_JS_SOURCE_SPECS + [("css", "css"), ("scss", "scss"), ("less", "less"), ("html", "html"), ("graphql", ["graphql", "gql"]), ("json5", "json5"), ("jsonc", "jsonc")]

_javascript_library_forward = dx_library_forward_rule(
    provides = _DX_JS_LIBRARY_PROVIDES,
    required_providers = [(_JsInfo, "JsInfo")],
    quality_specs = _DX_JS_LIBRARY_SOURCE_SPECS,
    what = "javascript_*",
    allow_files = _JS_LIBRARY_EXTS,
    upstream_providers = [[_JsInfo]],
)

_javascript_binary_forward = dx_executable_forward_rule(
    kind = "executable",
    provides = DX_EXEC_PROVIDES,
    required_providers = [],
    quality_specs = _DX_JS_SOURCE_SPECS,
    what = "javascript_*",
    allow_files = _JS_EXTS,
    upstream_providers = [[DefaultInfo]],
    runtime = "besteffort",
)

def _javascript_wrap_library(name, srcs, visibility = None, **kwargs):
    dx_wrap(name, _js_library, _javascript_library_forward, srcs, visibility = visibility, **kwargs)

def javascript_binary_upstream_data(srcs, data):
    """Computes the upstream data inputs for one javascript_binary."""
    return list(srcs or []) + list(data or [])

def _javascript_wrap_binary(name, srcs, visibility = None, **kwargs):
    upstream_kwargs = dx_binary_upstream_kwargs(kwargs, None)
    upstream_kwargs.pop("aspect_hints", None)
    if len(srcs) > 0:
        upstream_kwargs["data"] = javascript_binary_upstream_data(srcs, upstream_kwargs.get("data"))
    _js_binary(
        name = name + "_upstream",
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
    """Minimal wrapper over js_library."""
    _javascript_wrap_library(name, srcs, visibility = visibility, **kwargs)

def _javascript_wasm_bindgen_library_impl(ctx):
    """Adapts one browser rust_binary to JsInfo without rerunning bindgen."""
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
    doc = "Adapts one browser rust_binary to JsInfo.",
    provides = [_JsInfo, DefaultInfo, _RustWasmBindgenInfo],
    attrs = {
        "bindgen": attr.label(
            doc = "rust_binary(wasm = \"browser\") target whose output is adapted.",
            mandatory = True,
            allow_files = True,
        ),
        "deps": attr.label_list(
            doc = "JsInfo dependencies whose runfiles are added to this target.",
            providers = [[_JsInfo]],
        ),
    },
)

def javascript_wasm_bindgen_library(name, bindgen, deps = None, visibility = None, **kwargs):
    """Adapts one browser rust_binary to JsInfo without rerunning bindgen."""
    _javascript_wasm_bindgen_library(
        name = name,
        bindgen = bindgen,
        deps = deps or [],
        visibility = visibility,
        **kwargs
    )

def javascript_binary(name, *, srcs = None, main = None, visibility = None, **kwargs):
    """Minimal wrapper over js_binary. main is the entry file; srcs are added to data."""
    effective_srcs = srcs if srcs != None else []
    if main != None:
        kwargs["entry_point"] = main
    _javascript_wrap_binary(name, effective_srcs, visibility = visibility, **kwargs)

_javascript_test = node_test_forward_rule("javascript_*", _JS_EXTS, _DX_JS_SOURCE_SPECS)

def javascript_test_rejection(runner, node_modules, main, kwargs):
    """Returns the rejection for bad javascript_test arguments, or None."""
    return node_test_runner_rejection("javascript_test", runner, node_modules, main, kwargs)

def javascript_test(name, *, srcs = None, main = None, runner = "jest", node_modules = None, data = None, package_json = None, visibility = None, env_inherit = None, **kwargs):
    """Minimal wrapper over jest_test, or js_test with runner = "node". runner defaults to "jest"; main is linted unless it is an external label."""
    rejection = javascript_test_rejection(runner, node_modules, main, kwargs)
    if rejection != None:
        fail(rejection)
    if runner == "node":
        upstream = _js_test
        upstream_kwargs = {"entry_point": main}
    else:
        upstream = _jest_test
        upstream_kwargs = node_jest_upstream_attrs(node_modules, kwargs)
    quality_srcs = dx_entry_quality_srcs(main, srcs)
    a = node_test_wrap_args(name, quality_srcs, list(srcs or []) + (list(data) if data != None else []), package_json, upstream_kwargs, env_inherit, kwargs)
    dx_wrap_test(name, upstream, _javascript_test, None, visibility = visibility, quality_srcs = quality_srcs, upstream_kwargs = a.upstream_kwargs, extra_forward_kwargs = a.extra_forward_kwargs, **a.kwargs)

def _source_label(src):
    return src if src.startswith(":") or src.startswith("//") or src.startswith("@") else ":" + src

def javascript_bench(name, *, srcs, main = None, node_modules, data = None, args = None, tags = None, **kwargs):
    """A javascript_test run by dx bench. `main`, the first of srcs by default, exports a tinybench Bench as its default export; its results land in dx-bench.json. `node_modules` must link tinybench."""
    javascript_test(
        name = name,
        srcs = srcs,
        runner = "node",
        main = Label("//javascript/private:bench_driver"),
        data = list(data or []) + [node_modules],
        args = ["$(rootpath %s)" % _source_label(main or srcs[0])] + list(args or []),
        tags = merge_tags(tags, "dx-bench", "manual"),
        **with_default_size(kwargs)
    )

def javascript_fuzz_test(name, *, srcs, main = None, node_modules, data = None, visibility = None, tags = None, **kwargs):
    """A Jazzer.js fuzz target plus a `<name>_run` runner for `dx fuzz`. `main`, the first of srcs by default, exports `fuzz(data)`. `node_modules` must link @jazzer.js/core."""
    kwargs = dict(kwargs)
    fixed_args = ["$(rootpath %s)" % _source_label(main or srcs[0])] + list(kwargs.pop("fixed_args", []))
    dx_fuzz_test(
        name,
        javascript_library,
        {"srcs": srcs},
        tags = tags,
        visibility = visibility,
        fuzz_rule = javascript_binary,
        fuzz_kwargs = {
            "main": Label("//javascript/private:fuzz_driver"),
            "data": [":" + name + "_bin", node_modules] + list(srcs) + list(data or []),
            "fixed_args": fixed_args,
        },
        **kwargs
    )
