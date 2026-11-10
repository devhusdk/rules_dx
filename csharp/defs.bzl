"""Public C# API."""

load(
    "//csharp/private:defs.bzl",
    _csharp_bench = "csharp_bench",
    _csharp_binary = "csharp_binary",
    _csharp_fuzz_test = "csharp_fuzz_test",
    _csharp_library = "csharp_library",
    _csharp_proto_library = "csharp_proto_library",
    _csharp_test = "csharp_test",
)

csharp_bench = _csharp_bench
csharp_binary = _csharp_binary
csharp_fuzz_test = _csharp_fuzz_test
csharp_library = _csharp_library
csharp_proto_library = _csharp_proto_library
csharp_test = _csharp_test
