"""Public Zig API."""

load(
    "//zig/private:defs.bzl",
    _zig_bench = "zig_bench",
    _zig_binary = "zig_binary",
    _zig_import_name = "zig_import_name",
    _zig_library = "zig_library",
    _zig_test = "zig_test",
)

zig_import_name = _zig_import_name
zig_library = _zig_library
zig_binary = _zig_binary
zig_test = _zig_test
zig_bench = _zig_bench
