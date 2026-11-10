"""Public haskell API."""

load(
    "//haskell/private:defs.bzl",
    _DEFAULT_GHCOPTS = "DEFAULT_GHCOPTS",
    _GMP = "GMP",
    _HASKELL_CELLS = "HASKELL_CELLS",
    _HASKELL_COMPATIBLE_WITH = "HASKELL_COMPATIBLE_WITH",
    _HaskellInfo = "HaskellInfo",
    _HaskellLibraryInfo = "HaskellLibraryInfo",
    _STANDARD_GHCOPTS = "STANDARD_GHCOPTS",
    _STRICT_GHCOPTS = "STRICT_GHCOPTS",
    _haskell_bench = "haskell_bench",
    _haskell_binary = "haskell_binary",
    _haskell_library = "haskell_library",
    _haskell_test = "haskell_test",
    _with_gmp = "with_gmp",
)

DEFAULT_GHCOPTS = _DEFAULT_GHCOPTS
GMP = _GMP
HASKELL_CELLS = _HASKELL_CELLS
HASKELL_COMPATIBLE_WITH = _HASKELL_COMPATIBLE_WITH
HaskellInfo = _HaskellInfo
HaskellLibraryInfo = _HaskellLibraryInfo
STANDARD_GHCOPTS = _STANDARD_GHCOPTS
STRICT_GHCOPTS = _STRICT_GHCOPTS
haskell_bench = _haskell_bench
haskell_binary = _haskell_binary
haskell_library = _haskell_library
haskell_test = _haskell_test
with_gmp = _with_gmp
