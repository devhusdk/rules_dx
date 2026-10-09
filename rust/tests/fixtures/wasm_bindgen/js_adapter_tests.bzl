"""Adapter proofs: JsInfo parity with RustWasmBindgenInfo and no own actions."""

load("@aspect_rules_js//js:providers.bzl", "JsInfo")
load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_rust_wasm_bindgen//:providers.bzl", "RustWasmBindgenInfo")

def _parity_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[JsInfo]
    reference = ctx.attr.reference[RustWasmBindgenInfo]
    want_sources = sorted([f.short_path for f in [reference.wasm, reference.snippets] + reference.js.to_list()])
    want_types = sorted([f.short_path for f in reference.ts.to_list()])
    got_sources = sorted([f.short_path for f in info.sources.to_list()])
    got_types = sorted([f.short_path for f in info.types.to_list()])
    asserts.equals(env, want_sources, got_sources, "JsInfo.sources must carry every bindgen output")
    asserts.equals(env, want_types, got_types, "JsInfo.types must carry the bindgen declarations")
    transitive = [f.short_path for f in info.transitive_sources.to_list()]
    for path in want_sources:
        asserts.true(env, path in transitive, "transitive_sources drops " + path)
    transitive_types = [f.short_path for f in info.transitive_types.to_list()]
    for path in want_types:
        asserts.true(env, path in transitive_types, "transitive_types drops " + path)
    return analysistest.end(env)

_parity_test = analysistest.make(
    _parity_impl,
    attrs = {
        "reference": attr.label(
            doc = "Bindgen target whose outputs define the expected sets.",
            mandatory = True,
        ),
    },
)

def wasm_bindgen_js_parity_test(name, subject, reference, **kwargs):
    """Instantiates the JsInfo parity proof for one adapter target."""
    kwargs.setdefault("size", "small")
    _parity_test(
        name = name,
        target_under_test = subject,
        reference = reference,
        **kwargs
    )

def _no_actions_impl(ctx):
    env = analysistest.begin(ctx)
    actions = analysistest.target_actions(env)
    asserts.equals(env, [], actions, "the adapter must register no actions of its own")
    return analysistest.end(env)

_no_actions_test = analysistest.make(_no_actions_impl)

def wasm_bindgen_js_no_actions_test(name, subject, **kwargs):
    """Instantiates the proof that one adapter target creates no actions."""
    kwargs.setdefault("size", "small")
    _no_actions_test(
        name = name,
        target_under_test = subject,
        **kwargs
    )
