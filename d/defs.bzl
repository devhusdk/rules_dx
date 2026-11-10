"""Public D API."""

load(
    "//d/private:defs.bzl",
    _D_COMPATIBLE_WITH = "D_COMPATIBLE_WITH",
    _d_bench = "d_bench",
    _d_binary = "d_binary",
    _d_fuzz_test = "d_fuzz_test",
    _d_library = "d_library",
    _d_test = "d_test",
)

D_COMPATIBLE_WITH = _D_COMPATIBLE_WITH
d_library = _d_library
d_binary = _d_binary
d_test = _d_test
d_fuzz_test = _d_fuzz_test
d_bench = _d_bench
