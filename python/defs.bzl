"""Public Python API."""

load(
    "//python/private:defs.bzl",
    _python_bench = "python_bench",
    _python_binary = "python_binary",
    _python_fuzz_test = "python_fuzz_test",
    _python_grpc_library = "python_grpc_library",
    _python_library = "python_library",
    _python_main = "python_main",
    _python_main_rejection = "python_main_rejection",
    _python_test = "python_test",
    _python_test_rejection = "python_test_rejection",
)

python_bench = _python_bench
python_binary = _python_binary
python_fuzz_test = _python_fuzz_test
python_grpc_library = _python_grpc_library
python_library = _python_library
python_main = _python_main
python_main_rejection = _python_main_rejection
python_test = _python_test
python_test_rejection = _python_test_rejection
