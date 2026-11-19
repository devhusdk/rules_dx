"""Public TypeScript API."""

load("//tools/docs/typescript:defs.bzl", _typescript_doc_shard = "typescript_doc_shard")
load(
    "//typescript/private:defs.bzl",
    _typescript_config = "typescript_config",
    _typescript_library = "typescript_library",
    _typescript_proto_library = "typescript_proto_library",
    _typescript_binary = "typescript_binary",
    _typescript_test = "typescript_test",
    _typescript_bench = "typescript_bench",
    _typescript_fuzz_test = "typescript_fuzz_test",
    _typescript_playwright_test = "typescript_playwright_test",
)

typescript_library = _typescript_library
typescript_proto_library = _typescript_proto_library
typescript_binary = _typescript_binary
typescript_test = _typescript_test
typescript_bench = _typescript_bench
typescript_fuzz_test = _typescript_fuzz_test
typescript_playwright_test = _typescript_playwright_test
typescript_config = _typescript_config
typescript_doc_shard = _typescript_doc_shard
