"""Hand-built RustWasmBindgenInfo targets for adapter negative proofs."""

load("@rules_rust_wasm_bindgen//:providers.bzl", "RustWasmBindgenInfo")

def _fake_bindgen_impl(ctx):
    """Publishes the declared files as one RustWasmBindgenInfo."""
    return [RustWasmBindgenInfo(
        js = depset(ctx.files.js),
        root = ctx.label.name,
        snippets = ctx.file.snippets,
        ts = depset(ctx.files.ts),
        wasm = ctx.file.wasm,
    )]

fake_bindgen = rule(
    implementation = _fake_bindgen_impl,
    doc = "Declares RustWasmBindgenInfo from arbitrary files for negative proofs.",
    attrs = {
        "js": attr.label_list(
            allow_files = True,
            doc = "JavaScript outputs.",
        ),
        "snippets": attr.label(
            allow_single_file = True,
            doc = "Snippets directory output.",
        ),
        "ts": attr.label_list(
            allow_files = True,
            doc = "TypeScript declaration outputs.",
        ),
        "wasm": attr.label(
            allow_single_file = True,
            doc = "Wasm module output.",
        ),
    },
    provides = [RustWasmBindgenInfo],
)
