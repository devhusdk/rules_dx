"""Public Scala API."""

load(
    "//scala/private:defs.bzl",
    _scala_library = "scala_library",
    _scala_binary = "scala_binary",
    _scala_test = "scala_test",
    _scala_fuzz_test = "scala_fuzz_test",
    _scala_bench = "scala_bench",
)
load("//tools/docs/jvm:defs.bzl", _scala_doc_shard = "scala_doc_shard")

scala_library = _scala_library
scala_binary = _scala_binary
scala_test = _scala_test
scala_fuzz_test = _scala_fuzz_test
scala_bench = _scala_bench
scala_doc_shard = _scala_doc_shard
