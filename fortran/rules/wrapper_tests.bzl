"""Wrapper conformance tests for Fortran."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal", "expect_true")
load(":defs.bzl", "FORTRAN_COMPATIBLE_WITH", "FORTRAN_EXTENSIONS", "library_kwargs", "library_name", "link_kwargs", "with_defaults")

UPSTREAM_EXTENSIONS = [".f", ".f90", ".f95", ".f03", ".f08", ".F", ".F90", ".F95", ".F03", ".F08"]

_ALL = {
    "args": ["--x"],
    "copts": ["-O2"],
    "data": [":d"],
    "defines": ["X"],
    "deps": [":lib"],
    "env": {"K": "1"},
    "env_inherit": ["HOME"],
    "hdrs": ["a.inc"],
    "includes": ["inc"],
    "linkopts": ["-lm"],
    "size": "small",
    "srcs": ["main.f90"],
    "stamp": 0,
    "tags": ["t"],
    "timeout": "short",
}

def fortran_wrapper_contract_tests(name):
    """Instantiates Fortran wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["fortran"],
        extra_checks = [
            expect_equal("the extensions match rules_fortran", ["." + ext for ext in FORTRAN_EXTENSIONS], UPSTREAM_EXTENSIONS),
            expect_equal("lint and format both run on Fortran", with_defaults({"tags": ["x"]})["tags"], ["x"]),
            expect_true("the Fortran cell is the default", with_defaults({})["target_compatible_with"] == FORTRAN_COMPATIBLE_WITH),
            expect_equal("an explicit compatibility is kept", with_defaults({"target_compatible_with": []})["target_compatible_with"], []),
            expect_equal(
                "compile attributes reach the library",
                sorted(library_kwargs(_ALL).keys()),
                ["copts", "defines", "deps", "hdrs", "includes", "srcs", "tags"],
            ),
            expect_equal(
                "link and run attributes reach the C++ rule",
                sorted(link_kwargs(_ALL).keys()),
                ["args", "data", "env", "env_inherit", "linkopts", "size", "stamp", "tags", "timeout"],
            ),
            expect_equal("configurable copts stay a select", type(library_kwargs({"copts": select({"//conditions:default": ["-O2"]})})["copts"]), "select"),
            expect_equal("configurable linkopts stay a select", type(link_kwargs({"linkopts": select({"//conditions:default": ["-lm"]})})["linkopts"]), "select"),
            expect_equal("the private library is named after the executable", library_name("app_upstream"), "app_upstream_fortran"),
        ],
    )
