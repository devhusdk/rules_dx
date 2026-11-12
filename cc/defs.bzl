"""Public C and C++ API."""

load(
    "//cc/private:defs.bzl",
    _cc_bench = "cc_bench",
    _cc_binary = "cc_binary",
    _cc_coverage_settings = "cc_coverage_settings",
    _cc_fuzz_test = "cc_fuzz_test",
    _cc_library = "cc_library",
    _cc_test = "cc_test",
    _objc_library = "objc_library",
)

cc_bench = _cc_bench
cc_binary = _cc_binary
cc_coverage_settings = _cc_coverage_settings
cc_fuzz_test = _cc_fuzz_test
cc_library = _cc_library
cc_test = _cc_test
objc_library = _objc_library
