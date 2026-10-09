"""A consumer-owned bundle of rules_dx wasm outputs."""

_BUILD_BAZEL = '''genrule(
    name = "bundle_raw",
    srcs = ["@rules_dx//rust/tests/fixtures/wasm_bindgen:wasm_hello_raw"],
    outs = ["raw.wasm"],
    cmd = "cp $(location @rules_dx//rust/tests/fixtures/wasm_bindgen:wasm_hello_raw) $@",
    visibility = ["//visibility:public"],
)

genrule(
    name = "bundle_web_js",
    srcs = ["@rules_dx//rust/tests/fixtures/wasm_bindgen:wasm_hello_web"],
    outs = ["web.js"],
    cmd = """
        for f in $(locations @rules_dx//rust/tests/fixtures/wasm_bindgen:wasm_hello_web); do
          case "$$f" in
            *_bg.js) ;;
            *.js) cp "$$f" $@ ;;
          esac
        done
    """,
    visibility = ["//visibility:public"],
)

genrule(
    name = "bundle_web_wasm",
    srcs = ["@rules_dx//rust/tests/fixtures/wasm_bindgen:wasm_hello_web"],
    outs = ["web_bg.wasm"],
    cmd = """
        for f in $(locations @rules_dx//rust/tests/fixtures/wasm_bindgen:wasm_hello_web); do
          case "$$f" in
            *_bg.wasm) cp "$$f" $@ ;;
          esac
        done
    """,
    visibility = ["//visibility:public"],
)
'''

def _consumer_wasm_bindgen_repo_impl(ctx):
    """Writes the bundle an independent consumer of rules_dx wasm would own."""
    ctx.file("BUILD.bazel", _BUILD_BAZEL)

consumer_wasm_bindgen_repo = repository_rule(
    implementation = _consumer_wasm_bindgen_repo_impl,
)
