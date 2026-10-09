"""Wasm-bindgen outputs republished as JsInfo."""

load("@aspect_rules_js//js:providers.bzl", "JsInfo", "js_info")
load("@rules_rust_wasm_bindgen//:providers.bzl", "RustWasmBindgenInfo")

def _rust_wasm_bindgen_js_impl(ctx):
    """Republishes one bindgen target's outputs as JsInfo and adds no actions."""
    info = ctx.attr.bindings[RustWasmBindgenInfo]
    if info.wasm == None:
        fail("rust_wasm_bindgen_js: bindings target '{}' produced no .wasm output".format(ctx.attr.bindings.label))
    if info.snippets == None:
        fail("rust_wasm_bindgen_js: bindings target '{}' produced no snippets directory".format(ctx.attr.bindings.label))
    js = info.js.to_list()
    if not js:
        fail("rust_wasm_bindgen_js: bindings target '{}' produced no JavaScript outputs".format(ctx.attr.bindings.label))
    ts = info.ts.to_list()
    if ctx.attr.require_types and not ts:
        fail("rust_wasm_bindgen_js: bindings target '{}' produced no TypeScript declarations; bind with TypeScript enabled or set require_types = False".format(ctx.attr.bindings.label))
    sources = [info.wasm, info.snippets] + js
    files = sources + ts
    return [
        DefaultInfo(
            files = depset(files),
            runfiles = ctx.runfiles(files = files),
        ),
        js_info(
            target = ctx.label,
            sources = depset(sources),
            types = depset(ts),
            transitive_sources = depset(sources, transitive = [dep[JsInfo].transitive_sources for dep in ctx.attr.deps]),
            transitive_types = depset(ts, transitive = [dep[JsInfo].transitive_types for dep in ctx.attr.deps]),
            npm_sources = depset(transitive = [dep[JsInfo].npm_sources for dep in ctx.attr.deps]),
            npm_package_store_infos = depset(transitive = [dep[JsInfo].npm_package_store_infos for dep in ctx.attr.deps]),
        ),
    ]

rust_wasm_bindgen_js = rule(
    implementation = _rust_wasm_bindgen_js_impl,
    doc = "Republishes rust_wasm_bindgen outputs as JsInfo without regenerating bindings.",
    attrs = {
        "bindings": attr.label(
            doc = "A rust_wasm_bindgen target providing RustWasmBindgenInfo.",
            mandatory = True,
            providers = [[RustWasmBindgenInfo]],
        ),
        "deps": attr.label_list(
            doc = "Existing JsInfo targets merged into the transitive fields.",
            providers = [JsInfo],
        ),
        "require_types": attr.bool(
            doc = "Fails analysis when the bindgen target produced no declarations.",
            default = False,
        ),
    },
    provides = [JsInfo],
)
