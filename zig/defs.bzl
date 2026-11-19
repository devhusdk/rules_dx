"""Public Zig API."""

load("//tools/docs/zig:defs.bzl", _zig_doc_shard = "zig_doc_shard")
load(
    "//zig/private:defs.bzl",
    _zig_bench = "zig_bench",
    _zig_binary = "zig_binary",
    _zig_library = "zig_library",
    _zig_test = "zig_test",
)

zig_library = _zig_library
zig_binary = _zig_binary
zig_test = _zig_test
zig_bench = _zig_bench
zig_doc_shard = _zig_doc_shard
