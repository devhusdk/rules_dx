"""Public Dart API."""

load(
    "//dart/private:defs.bzl",
    _dart_bench = "dart_bench",
    _dart_binary = "dart_binary",
    _dart_codegen = "dart_codegen",
    _dart_config = "dart_config",
    _dart_js_binary = "dart_js_binary",
    _dart_library = "dart_library",
    _dart_test = "dart_test",
    _dart_wasm_binary = "dart_wasm_binary",
)

dart_bench = _dart_bench
dart_binary = _dart_binary
dart_codegen = _dart_codegen
dart_config = _dart_config
dart_js_binary = _dart_js_binary
dart_library = _dart_library
dart_test = _dart_test
dart_wasm_binary = _dart_wasm_binary
