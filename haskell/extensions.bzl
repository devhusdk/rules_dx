"""Public Haskell module extensions."""

load("//haskell/private/toolchain:extensions.bzl", _cabal = "cabal", _haskell = "haskell")

cabal = _cabal
haskell = _haskell
