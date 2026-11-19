"""Public Java API."""

load(
    "//java/private:defs.bzl",
    _java_library = "java_library",
    _java_binary = "java_binary",
    _java_test = "java_test",
    _java_fuzz_test = "java_fuzz_test",
    _java_bench = "java_bench",
)
load("//tools/docs/jvm:defs.bzl", _java_doc_shard = "java_doc_shard")

java_library = _java_library
java_binary = _java_binary
java_test = _java_test
java_fuzz_test = _java_fuzz_test
java_bench = _java_bench
java_doc_shard = _java_doc_shard
