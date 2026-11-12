"""Public OCaml API."""

load(
    "//ocaml/private:defs.bzl",
    _ocaml_bench = "ocaml_bench",
    _ocaml_binary = "ocaml_binary",
    _ocaml_fuzz_test = "ocaml_fuzz_test",
    _ocaml_library = "ocaml_library",
    _ocaml_module = "ocaml_module",
    _ocaml_signature = "ocaml_signature",
    _ocaml_test = "ocaml_test",
    _ocaml_lex = "ocaml_lex",
)
load("//ocaml/private:dune.bzl", _ocaml_dune_library = "ocaml_dune_library")
load("//ocaml/private:providers.bzl", _OCamlCmtInfo = "OCamlCmtInfo")

OCamlCmtInfo = _OCamlCmtInfo
ocaml_dune_library = _ocaml_dune_library
ocaml_bench = _ocaml_bench
ocaml_binary = _ocaml_binary
ocaml_fuzz_test = _ocaml_fuzz_test
ocaml_library = _ocaml_library
ocaml_module = _ocaml_module
ocaml_signature = _ocaml_signature
ocaml_test = _ocaml_test
ocaml_lex = _ocaml_lex
