"""Public Haskell API."""

load(
    "//haskell/private:defs.bzl",
    _haskell_bench = "haskell_bench",
    _haskell_binary = "haskell_binary",
    _haskell_library = "haskell_library",
    _haskell_test = "haskell_test",
)

haskell_bench = _haskell_bench
haskell_binary = _haskell_binary
haskell_library = _haskell_library
haskell_test = _haskell_test
