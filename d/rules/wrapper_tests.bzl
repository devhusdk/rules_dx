"""Wrapper conformance tests for D."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "D_COMPATIBLE_WITH", "d_quality_srcs")

def d_wrapper_contract_tests(name):
    """Instantiates D wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["d"],
        extra_checks = [
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
                "D targets build on linux x86_64 only",
                [str(constraint) for constraint in D_COMPATIBLE_WITH],
                [str(Label("@platforms//os:linux")), str(Label("@platforms//cpu:x86_64"))],
            ),
        ],
    )
