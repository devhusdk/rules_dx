"""Wrapper conformance tests for Julia."""

load("//libs/starlark:conformance.bzl", "dx_wrapper_contract_tests")
load("//libs/starlark:defs.bzl", "expect_equal")
load(":defs.bzl", "JULIA_COVERAGE_COLLECTOR", "JULIA_QUALITY_TOOLS", "JULIA_SOURCE_EXTS", "JULIA_SOURCE_SPECS", "julia_quality_srcs")
load(":platforms.bzl", "JULIA_PLATFORMS")

def julia_wrapper_contract_tests(name):
    """Instantiates Julia wrapper contract tests."""
    dx_wrapper_contract_tests(
        name,
        file_classes = ["julia"],
        extra_checks = [
            expect_equal(
                "wrappers carry julia_format",
                sorted(JULIA_QUALITY_TOOLS.keys()),
                ["julia_format"],
            ),
            expect_equal(
                "one julia class owns every source",
                JULIA_SOURCE_SPECS,
                [("julia", "jl")],
            ),
            expect_equal(
                "wrappers accept only Julia sources",
                JULIA_SOURCE_EXTS,
                [".jl"],
            ),
            expect_equal(
                "main leads the owned sources",
                julia_quality_srcs("main.jl", ["util.jl"]),
                ["main.jl", "util.jl"],
            ),
            expect_equal(
                "main listed in srcs is owned once",
                julia_quality_srcs("main.jl", ["main.jl", "util.jl"]),
                ["main.jl", "util.jl"],
            ),
            expect_equal(
                "no main and no srcs owns nothing",
                julia_quality_srcs(None, None),
                [],
            ),
            expect_equal(
                "julia tests have no coverage collector yet",
                JULIA_COVERAGE_COLLECTOR,
                False,
            ),
            expect_equal(
                "windows arm64 has no Julia build",
                [name for name, (os, cpu) in JULIA_PLATFORMS.items() if os == "windows" and cpu == "arm64"],
                [],
            ),
        ],
    )
