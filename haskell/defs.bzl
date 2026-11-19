"""Public Haskell API."""

load(
    "//haskell/private:defs.bzl",
    _haskell_bench = "haskell_bench",
    _haskell_binary = "haskell_binary",
    _haskell_library = "haskell_library",
    _haskell_test = "haskell_test",
)
load("//tools/docs/haskell:defs.bzl", _haskell_doc_shard = "haskell_doc_shard")

haskell_bench = _haskell_bench
haskell_binary = _haskell_binary
haskell_library = _haskell_library
haskell_test = _haskell_test
haskell_doc_shard = _haskell_doc_shard
