"""Public D API."""

load(
    "//d/private:defs.bzl",
    _d_bench = "d_bench",
    _d_binary = "d_binary",
    _d_fuzz_test = "d_fuzz_test",
    _d_library = "d_library",
    _d_test = "d_test",
)
load("//tools/docs/d:defs.bzl", _d_doc_shard = "d_doc_shard")

d_library = _d_library
d_binary = _d_binary
d_test = _d_test
d_fuzz_test = _d_fuzz_test
d_bench = _d_bench
d_doc_shard = _d_doc_shard
