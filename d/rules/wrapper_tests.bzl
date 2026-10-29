"""Wrapper conformance tests for D."""

load("//d/toolchain:pins.bzl", "LDC_CELLS", "LDC_UNPROVEN_CELLS")
load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "D_CELLS", "D_QUALITY_TOOLS", "d_quality_srcs")

def d_wrapper_contract_tests(name):
    """Instantiates D wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["d"],
        extra_checks = [
            expect_equal(
                "wrappers carry dfmt and dscanner",
                sorted(D_QUALITY_TOOLS.keys()),
                ["dfmt", "dscanner"],
            ),
            expect_equal(
                "modules and interface files are owned in order",
                d_quality_srcs(["app.d", "api.di"]),
                ["app.d", "api.di"],
            ),
            expect_equal(
                "string import files are not owned",
                d_quality_srcs(["app.d", "banner.txt"], ["banner.txt"]),
                ["app.d"],
            ),
            expect_equal(
                "no sources own nothing",
                d_quality_srcs(None),
                [],
            ),
            expect_equal(
                "D targets build on every proven LDC cell",
                D_CELLS,
                sorted([cell for cell in LDC_CELLS.keys() if cell not in LDC_UNPROVEN_CELLS]),
            ),
        ],
    )
