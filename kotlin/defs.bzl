"""Public Kotlin API."""

load(
    "//kotlin/private:defs.bzl",
    _kotlin_library = "kotlin_library",
    _kotlin_binary = "kotlin_binary",
    _kotlin_test = "kotlin_test",
    _kotlin_bench = "kotlin_bench",
    _kotlin_fuzz_test = "kotlin_fuzz_test",
)
load("//tools/docs/jvm:defs.bzl", _kotlin_doc_shard = "kotlin_doc_shard")

kotlin_library = _kotlin_library
kotlin_binary = _kotlin_binary
kotlin_test = _kotlin_test
kotlin_bench = _kotlin_bench
kotlin_fuzz_test = _kotlin_fuzz_test
kotlin_doc_shard = _kotlin_doc_shard
