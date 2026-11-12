"""Public C and C++ API."""

load(
    "//cc/private:defs.bzl",
    _cc_bench = "cc_bench",
    _cc_binary = "cc_binary",
    _cc_fuzz_test = "cc_fuzz_test",
    _cc_library = "cc_library",
    _cc_test = "cc_test",
    _cc_objc_library = "cc_objc_library",
)

cc_bench = _cc_bench
cc_binary = _cc_binary
cc_fuzz_test = _cc_fuzz_test
cc_library = _cc_library
cc_test = _cc_test
cc_objc_library = _cc_objc_library
