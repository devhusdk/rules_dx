"""Public Fortran API."""

load("//fortran/private/tools:fpm_lock.bzl", _fpm_lock = "fpm_lock")
load(
    "//fortran/private:defs.bzl",
    _fortran_binary = "fortran_binary",
    _fortran_library = "fortran_library",
    _fortran_test = "fortran_test",
)

fortran_library = _fortran_library
fortran_binary = _fortran_binary
fortran_test = _fortran_test
fpm_lock = _fpm_lock
