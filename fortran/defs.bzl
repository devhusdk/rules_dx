"""Public Fortran API."""

load(
    "//fortran/private:defs.bzl",
    _FORTRAN_COMPATIBLE_WITH = "FORTRAN_COMPATIBLE_WITH",
    _fortran_binary = "fortran_binary",
    _fortran_library = "fortran_library",
    _fortran_test = "fortran_test",
    _library_name = "library_name",
)

FORTRAN_COMPATIBLE_WITH = _FORTRAN_COMPATIBLE_WITH
library_name = _library_name
fortran_library = _fortran_library
fortran_binary = _fortran_binary
fortran_test = _fortran_test
