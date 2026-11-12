"""Public Haskell API."""

load("//haskell/private:deps_resolve.bzl", _haskell_deps_resolve = "haskell_deps_resolve", _haskell_deps_resolve_test = "haskell_deps_resolve_test")
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
haskell_deps_resolve = _haskell_deps_resolve
haskell_deps_resolve_test = _haskell_deps_resolve_test
