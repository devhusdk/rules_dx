"""Parity observations between the owned wasm JsInfo adapter and upstream outputs."""

load("@aspect_rules_js//js:providers.bzl", "JsInfo")
load("@rules_rust_wasm_bindgen//:providers.bzl", "RustWasmBindgenInfo")
load("//libs/starlark:defs.bzl", "DxSubjectInfo", "starlark_test")

def _sorted_paths(files):
    """Returns the sorted short paths of one file list."""
    return sorted([f.short_path for f in files])

def _wasm_js_parity_subject_impl(ctx):
    """Exposes adapter-versus-upstream closure comparisons for one binding."""
    adapter = ctx.attr.adapter
    upstream = ctx.attr.upstream
    adapted = adapter[JsInfo]
    bound = upstream[RustWasmBindgenInfo]
    adapter_files = _sorted_paths(adapter[DefaultInfo].files.to_list())
    upstream_files = _sorted_paths(upstream[DefaultInfo].files.to_list())
    adapter_sources = _sorted_paths(adapted.sources.to_list())
    bindgen_js = _sorted_paths(bound.js.to_list())
    adapter_types = _sorted_paths(adapted.types.to_list())
    bindgen_ts = _sorted_paths(bound.ts.to_list())
    transitive = _sorted_paths(adapted.transitive_sources.to_list())
    transitive_types = _sorted_paths(adapted.transitive_types.to_list())
    wanted = _sorted_paths(ctx.files.expected_transitives)
    missing = [path for path in wanted if path not in transitive and path not in transitive_types]
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = {
            "files.match_upstream": str(adapter_files == upstream_files),
            "snippets.in_files": str(bound.snippets.short_path in adapter_files),
            "sources.match_bindgen": str(adapter_sources == bindgen_js),
            "transitive.complete": str(missing == []),
            "types.match_bindgen": str(adapter_types == bindgen_ts),
            "wasm.in_files": str(bound.wasm.short_path in adapter_files),
        }),
    ]

wasm_js_parity_subject = rule(
    implementation = _wasm_js_parity_subject_impl,
    attrs = {
        "adapter": attr.label(mandatory = True, providers = [JsInfo]),
        "expected_transitives": attr.label_list(allow_files = True),
        "upstream": attr.label(mandatory = True, providers = [RustWasmBindgenInfo]),
    },
)

def _synthetic_bindgen_impl(ctx):
    """Returns one RustWasmBindgenInfo with the flavor-selected output missing."""
    dummy = ctx.actions.declare_file(ctx.label.name + ".dummy.js")
    ctx.actions.write(output = dummy, content = "export const dummy = 1;\n")
    flavor = ctx.attr.flavor
    return [
        DefaultInfo(files = depset([dummy])),
        RustWasmBindgenInfo(
            js = depset() if flavor == "missing_js" else depset([dummy]),
            root = "dummy",
            snippets = None if flavor == "missing_snippets" else dummy,
            ts = depset() if flavor == "missing_ts" else depset([dummy]),
            wasm = None if flavor == "missing_wasm" else dummy,
        ),
    ]

synthetic_bindgen = rule(
    implementation = _synthetic_bindgen_impl,
    attrs = {
        "flavor": attr.string(mandatory = True),
    },
)

def wasm_js_parity_tests(name, subjects):
    """Instantiates the golden adapter-parity test over parity subjects."""
    package = "//rust/tests/fixtures/wasm_bindgen"
    blocks = []
    for subject in sorted(subjects):
        blocks.append("\n".join([
            "subject " + package + ":" + subject,
            "field files.match_upstream=True",
            "field snippets.in_files=True",
            "field sources.match_bindgen=True",
            "field transitive.complete=True",
            "field types.match_bindgen=True",
            "field wasm.in_files=True",
            "aspect_field aspect_seen=True",
            "aspect_field field_count=6",
            "aspect_field has_subject=True",
            "aspect_field subject_label=" + package + ":" + subject,
            "aspect_field transitive_count=0",
        ]))
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":" + subject for subject in subjects],
        expected_observations = "\n".join(blocks),
    )
