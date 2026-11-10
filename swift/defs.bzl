"""Public Swift API."""

load(
    "//swift/private:defs.bzl",
    _SWIFT_WINDOWS_RUNTIME_DLLS = "SWIFT_WINDOWS_RUNTIME_DLLS",
    _SWIFT_WINDOWS_RUNTIME_PATH = "SWIFT_WINDOWS_RUNTIME_PATH",
    _swift_bench = "swift_bench",
    _swift_binary = "swift_binary",
    _swift_fuzz_test = "swift_fuzz_test",
    _swift_interop_hint = "swift_interop_hint",
    _swift_library = "swift_library",
    _swift_test = "swift_test",
)

SWIFT_WINDOWS_RUNTIME_DLLS = _SWIFT_WINDOWS_RUNTIME_DLLS
SWIFT_WINDOWS_RUNTIME_PATH = _SWIFT_WINDOWS_RUNTIME_PATH
swift_bench = _swift_bench
swift_binary = _swift_binary
swift_fuzz_test = _swift_fuzz_test
swift_interop_hint = _swift_interop_hint
swift_library = _swift_library
swift_test = _swift_test
