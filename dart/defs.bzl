"""Public Dart API."""

load(
    "//dart/private:defs.bzl",
    _DART_COMPATIBLE_WITH = "DART_COMPATIBLE_WITH",
    _DART_QUALITY_TOOLS = "DART_QUALITY_TOOLS",
    _DART_SOURCE_SPECS = "DART_SOURCE_SPECS",
    _DxDartConfigInfo = "DxDartConfigInfo",
    _dart_bench = "dart_bench",
    _dart_binary = "dart_binary",
    _dart_codegen = "dart_codegen",
    _dart_config = "dart_config",
    _dart_js_binary = "dart_js_binary",
    _dart_library = "dart_library",
    _dart_test = "dart_test",
    _dart_wasm_binary = "dart_wasm_binary",
)

DART_COMPATIBLE_WITH = _DART_COMPATIBLE_WITH
DART_QUALITY_TOOLS = _DART_QUALITY_TOOLS
DART_SOURCE_SPECS = _DART_SOURCE_SPECS
DxDartConfigInfo = _DxDartConfigInfo
dart_bench = _dart_bench
dart_binary = _dart_binary
dart_codegen = _dart_codegen
dart_config = _dart_config
dart_js_binary = _dart_js_binary
dart_library = _dart_library
dart_test = _dart_test
dart_wasm_binary = _dart_wasm_binary
