"""Wrapper conformance tests for Haskell."""

load("//haskell/toolchain:pins.bzl", "CELLS")
load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "DEFAULT_GHCOPTS", "GMP", "HASKELL_CELLS", "HASKELL_COMPATIBLE_WITH", "upstream_kwargs", "with_gmp")

def haskell_wrapper_contract_tests(name):
    """Instantiates Haskell wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["haskell"],
        extra_checks = [
            expect_equal("warnings fail the compile by default", DEFAULT_GHCOPTS, ["-Wall", "-Werror"]),
            expect_equal("the default ghcopts reach the compiler", upstream_kwargs({})["ghcopts"], ["-Wall", "-Werror"]),
            expect_equal("explicit ghcopts win", upstream_kwargs({"ghcopts": ["-O2"]})["ghcopts"], ["-O2"]),
            expect_equal("empty ghcopts win", upstream_kwargs({"ghcopts": []})["ghcopts"], []),
            expect_equal("other kwargs pass through", upstream_kwargs({"deps": [":a"]})["deps"], [":a"]),
            expect_equal("executables get gmp once", [str(dep) for dep in with_gmp([":a"])], [":a", str(GMP)]),
            expect_equal("a listed gmp is kept", [str(dep) for dep in with_gmp([str(GMP)])], [str(GMP)]),
            expect_equal("select deps get gmp appended", type(with_gmp([":a"] + select({"//conditions:default": []}))), "select"),
            expect_equal("no deps still links gmp", [str(dep) for dep in with_gmp(None)], [str(GMP)]),
            expect_equal("packages compile on every pinned GHC cell", HASKELL_CELLS, sorted(CELLS.keys())),
            expect_equal("compatibility is a select over the cells", type(HASKELL_COMPATIBLE_WITH), "select"),
        ],
    )
