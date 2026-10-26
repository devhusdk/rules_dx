"""Wrapper conformance tests for OCaml."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "launcher_script", "link_kwargs", "owned_sources", "runfiles_path", "test_upstream_kwargs", "toolchain_choice", "upstream_tags")

def ocaml_wrapper_contract_tests(name):
    """Instantiates OCaml wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["ocaml"],
        extra_checks = [
            expect_equal("the native default builds bytecode", toolchain_choice("ocamlopt.opt"), "ocamlc.opt"),
            expect_equal("an explicit toolchain is kept", toolchain_choice("ocamlc.byte"), "ocamlc.byte"),
            expect_equal("owned sources are .ml and .mli files", owned_sources("a.ml", ":sig", None, "b.mli"), ["a.ml", "b.mli"]),
            expect_equal("upstream actions stay local", upstream_tags(None)[-1], "no-remote-exec"),
            expect_equal("binaries link without a runtime", link_kwargs({"opts": ["-g"]}), {"opts": ["-g", "-without-runtime"], "vm_linkage": "dynamic"}),
            expect_equal("test attributes stay on the forwarder", test_upstream_kwargs({"main": ":m", "size": "small"}), {"main": ":m"}),
            expect_equal("main repo runfiles", runfiles_path("_main", "pkg/app.bc"), "_main/pkg/app.bc"),
            expect_equal("external runfiles", runfiles_path("_main", "../repo/bin/ocamlrun"), "repo/bin/ocamlrun"),
            expect_equal("launcher runs bytecode on ocamlrun", launcher_script("r", "b").splitlines()[-1], 'exec "$runfiles/r" "$runfiles/b" "$@"'),
        ],
    )
