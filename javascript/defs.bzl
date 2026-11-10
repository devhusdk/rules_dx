"""Public JavaScript API."""

load(
    "//javascript/private:defs.bzl",
    _javascript_binary_upstream_data = "javascript_binary_upstream_data",
    _javascript_library = "javascript_library",
    _javascript_wasm_bindgen_library = "javascript_wasm_bindgen_library",
    _javascript_binary = "javascript_binary",
    _javascript_test_rejection = "javascript_test_rejection",
    _javascript_test = "javascript_test",
    _javascript_bench = "javascript_bench",
    _javascript_fuzz_test = "javascript_fuzz_test",
)

load(
    "//javascript/private:web.bzl",
    _javascript_web_app = "javascript_web_app",
    _javascript_web_bundle = "javascript_web_bundle",
)

javascript_binary_upstream_data = _javascript_binary_upstream_data
javascript_library = _javascript_library
javascript_wasm_bindgen_library = _javascript_wasm_bindgen_library
javascript_binary = _javascript_binary
javascript_test_rejection = _javascript_test_rejection
javascript_test = _javascript_test
javascript_bench = _javascript_bench
javascript_fuzz_test = _javascript_fuzz_test
javascript_web_app = _javascript_web_app
javascript_web_bundle = _javascript_web_bundle
