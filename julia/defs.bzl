"""Public Julia API."""

load(
    "//julia/private:defs.bzl",
    _JULIA_COMPATIBLE_WITH = "JULIA_COMPATIBLE_WITH",
    _julia_binary = "julia_binary",
    _julia_library = "julia_library",
    _julia_standalone_binary = "julia_standalone_binary",
    _julia_test = "julia_test",
)

JULIA_COMPATIBLE_WITH = _JULIA_COMPATIBLE_WITH
julia_library = _julia_library
julia_binary = _julia_binary
julia_test = _julia_test
julia_standalone_binary = _julia_standalone_binary
