"""Public F# API."""

load(
    "//fsharp/private:defs.bzl",
    _fsharp_bench = "fsharp_bench",
    _fsharp_binary = "fsharp_binary",
    _fsharp_fuzz_test = "fsharp_fuzz_test",
    _fsharp_library = "fsharp_library",
    _fsharp_test = "fsharp_test",
)

fsharp_bench = _fsharp_bench
fsharp_binary = _fsharp_binary
fsharp_fuzz_test = _fsharp_fuzz_test
fsharp_library = _fsharp_library
fsharp_test = _fsharp_test
