"""Public C and C++ API."""

load(
    "//cc/private:defs.bzl",
    _CC_COVERAGE_INPUTS = "CC_COVERAGE_INPUTS",
    _CC_COVERAGE_TRANSITION = "CC_COVERAGE_TRANSITION",
    _cc_bench = "cc_bench",
    _cc_binary = "cc_binary",
    _cc_coverage_settings = "cc_coverage_settings",
    _cc_fuzz_test = "cc_fuzz_test",
    _cc_library = "cc_library",
    _cc_test = "cc_test",
    _objc_library = "objc_library",
)

CC_COVERAGE_INPUTS = _CC_COVERAGE_INPUTS
CC_COVERAGE_TRANSITION = _CC_COVERAGE_TRANSITION
cc_bench = _cc_bench
cc_binary = _cc_binary
cc_coverage_settings = _cc_coverage_settings
cc_fuzz_test = _cc_fuzz_test
cc_library = _cc_library
cc_test = _cc_test
objc_library = _objc_library
