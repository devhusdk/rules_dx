"""Wrapper conformance tests for OCaml."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "default_module_name", "launcher_script", "link_kwargs", "owned_sources", "run_environment", "runfiles_path", "test_forward_kwargs", "test_upstream_kwargs", "toolchain_choice", "upstream_tags", "with_defaults")
load(":providers.bzl", "bin_annot_opts", "cmt_entries")

def _F(basename):
    return struct(basename = basename)

def ocaml_wrapper_contract_tests(name):
    """Instantiates OCaml wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["ocaml"],
        extra_checks = [
            expect_equal("the native default builds bytecode", toolchain_choice("ocamlopt.opt", "vm"), "ocamlc.opt"),
            expect_equal("an explicit toolchain is kept", toolchain_choice("ocamlc.byte", "vm"), "ocamlc.byte"),
            expect_equal("the sys emitter builds native", toolchain_choice("ocamlopt.opt", "sys"), "ocamlopt.opt"),
            expect_equal("the sys emitter replaces the bytecode default", toolchain_choice("ocamlc.opt", "sys"), "ocamlopt.opt"),
            expect_equal("owned sources are .ml and .mli files", owned_sources("a.ml", ":sig", None, "b.mli"), ["a.ml", "b.mli"]),
            expect_equal("typecheck skips OCaml, format and lint do not", with_defaults({"tags": ["x"]})["tags"], ["x", "no-typecheck"]),
            expect_equal("upstream actions stay local", upstream_tags(None)[-1], "no-remote-exec"),
            expect_equal("binaries link without a runtime only for the vm emitter", type(link_kwargs({"opts": ["-g"]})["opts"]), "select"),
            expect_equal("binaries without opts get the emitter select", type(link_kwargs({})["opts"]), "select"),
            expect_equal("binaries keep the vm linkage", link_kwargs({})["vm_linkage"], "dynamic"),
            expect_equal("configurable opts stay a select", type(link_kwargs({"opts": select({"//conditions:default": ["-g"]})})["opts"]), "select"),
            expect_equal("test attributes stay on the forwarder", test_upstream_kwargs({"main": ":m", "size": "small"}), {"main": ":m"}),
            expect_equal("testonly is set by ocaml_test, not passed upstream", test_upstream_kwargs({"main": ":m", "testonly": True}), {"main": ":m"}),
            expect_equal("env and env_inherit leave the upstream binary", test_upstream_kwargs({"env": {"K": "1"}, "env_inherit": ["HOME"]}), {}),
            expect_equal("env and env_inherit reach the test forwarder", test_forward_kwargs({"env": {"K": "1"}, "env_inherit": ["HOME"]}), {"env": {"K": "1"}, "env_inherit": ["HOME"]}),
            expect_equal("the forwarder run environment merges upstream and wrapper env", run_environment({"A": "1", "K": "0"}, ["PATH"], {"K": "1"}, ["HOME", "PATH"]), struct(environment = {"A": "1", "K": "1"}, inherited_environment = ["PATH", "HOME"])),
            expect_equal("a .mli sig keeps the target module name", default_module_name("m", "m.mli"), "m"),
            expect_equal("a signature target sets the module name", default_module_name("m", ":m_sig"), None),
            expect_equal("a configurable sig leaves the module name upstream", default_module_name("m", select({"//conditions:default": "m.mli"})), None),
            expect_equal("a Label sig leaves the module name upstream", default_module_name("m", Label("//ocaml/rules:m.mli")), None),
            expect_equal("modules get -bin-annot", bin_annot_opts(None), ["-bin-annot"]),
            expect_equal("-bin-annot is appended once", bin_annot_opts(["-g", "-bin-annot"]), ["-g", "-bin-annot"]),
            expect_equal("-no-bin-annot opts out", bin_annot_opts(["-no-bin-annot"]), ["-no-bin-annot"]),
            expect_equal("a struct and sig expose both", cmt_entries([_F("m.ml"), _F("m.mli")], [_F("m.cmt")], [_F("m.cmti")])[0].cmti, _F("m.cmti")),
            expect_equal("the struct maps to its cmt", cmt_entries([_F("m.ml"), _F("m.mli")], [_F("m.cmt")], [_F("m.cmti")])[0].source, _F("m.ml")),
            expect_equal("the opt-out exposes nothing", cmt_entries([_F("m.ml")], [], []), []),
            expect_equal("a signature alone maps to its cmti", cmt_entries([_F("s.mli")], [], [_F("s.cmti")])[0].signature, _F("s.mli")),
            expect_equal("main repo runfiles", runfiles_path("_main", "pkg/app.bc"), "_main/pkg/app.bc"),
            expect_equal("external runfiles", runfiles_path("_main", "../repo/bin/ocamlrun"), "repo/bin/ocamlrun"),
            expect_equal("launcher runs bytecode on ocamlrun", launcher_script("r", "b").splitlines()[-1], 'exec "$runfiles/r" "$runfiles/b" "$@"'),
        ],
    )
