"""Public Fortran API."""

load(
    "//fortran/private:defs.bzl",
    _fortran_binary = "fortran_binary",
    _fortran_library = "fortran_library",
    _fortran_test = "fortran_test",
)
load("//tools/docs/fortran:defs.bzl", _fortran_doc_shard = "fortran_doc_shard")

fortran_library = _fortran_library
fortran_binary = _fortran_binary
fortran_test = _fortran_test
fortran_doc_shard = _fortran_doc_shard
