"""Public Go API."""

load(
    "//go/private:defs.bzl",
    _go_bench = "go_bench",
    _go_binary = "go_binary",
    _go_fuzz_test = "go_fuzz_test",
    _go_library = "go_library",
    _go_test = "go_test",
)
load("//tools/docs/go:defs.bzl", _go_doc_shard = "go_doc_shard")

go_bench = _go_bench
go_binary = _go_binary
go_fuzz_test = _go_fuzz_test
go_library = _go_library
go_test = _go_test
go_doc_shard = _go_doc_shard
