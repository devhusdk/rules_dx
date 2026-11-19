"""Public Python API."""

load(
    "//python/private:defs.bzl",
    _python_bench = "python_bench",
    _python_binary = "python_binary",
    _python_fuzz_test = "python_fuzz_test",
    _python_grpc_library = "python_grpc_library",
    _python_library = "python_library",
    _python_test = "python_test",
)
load("//tools/docs/python:defs.bzl", _python_doc_shard = "python_doc_shard")

python_bench = _python_bench
python_binary = _python_binary
python_fuzz_test = _python_fuzz_test
python_grpc_library = _python_grpc_library
python_library = _python_library
python_test = _python_test
python_doc_shard = _python_doc_shard
