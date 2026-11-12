"""Public Fortran module extensions."""

load("//fortran/private/lock:hub.bzl", _fpm = "fpm")
load("//fortran/private/toolchain:extensions.bzl", _fortran = "fortran")

fortran = _fortran
fpm = _fpm
