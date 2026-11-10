"""Public OCaml API."""

load(
    "//ocaml/private:defs.bzl",
    _OCAML_COMPATIBLE_WITH = "OCAML_COMPATIBLE_WITH",
    _launcher_script = "launcher_script",
    _ocaml_bench = "ocaml_bench",
    _ocaml_binary = "ocaml_binary",
    _ocaml_fuzz_test = "ocaml_fuzz_test",
    _ocaml_library = "ocaml_library",
    _ocaml_module = "ocaml_module",
    _ocaml_signature = "ocaml_signature",
    _ocaml_test = "ocaml_test",
    _ocamllex = "ocamllex",
    _owned_sources = "owned_sources",
    _runfiles_path = "runfiles_path",
    _toolchain_choice = "toolchain_choice",
    _toolchain_settings = "toolchain_settings",
)
load("//ocaml/private:dune.bzl", _dx_dune_library = "dx_dune_library")
load("//ocaml/private:providers.bzl", _OCamlCmtInfo = "OCamlCmtInfo")

OCAML_COMPATIBLE_WITH = _OCAML_COMPATIBLE_WITH
OCamlCmtInfo = _OCamlCmtInfo
dx_dune_library = _dx_dune_library
launcher_script = _launcher_script
ocaml_bench = _ocaml_bench
ocaml_binary = _ocaml_binary
ocaml_fuzz_test = _ocaml_fuzz_test
ocaml_library = _ocaml_library
ocaml_module = _ocaml_module
ocaml_signature = _ocaml_signature
ocaml_test = _ocaml_test
ocamllex = _ocamllex
owned_sources = _owned_sources
runfiles_path = _runfiles_path
toolchain_choice = _toolchain_choice
toolchain_settings = _toolchain_settings
