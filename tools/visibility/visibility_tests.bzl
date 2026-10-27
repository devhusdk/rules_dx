"""Unit tests for the shared visibility scopes."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(
    ":visibility.bzl",
    "VIS_CLI",
    "VIS_DX_COMMANDS",
    "VIS_DX_ROOTS",
    "VIS_PUBLIC",
    "VIS_QUALITY",
    "VIS_TOOLS",
    "VIS_TOOLS_CLI_TEST",
    "VIS_TOOLS_PUBLIC",
)

def visibility_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "public scope stays one entry",
                VIS_PUBLIC,
                ["//visibility:public"],
            ),
            expect_equal(
                "cli scope stays cli-only",
                VIS_CLI,
                ["//cli:__pkg__", "//cli:__subpackages__"],
            ),
            expect_equal(
                "quality scope stays quality-only",
                VIS_QUALITY,
                ["//quality:__pkg__", "//quality:__subpackages__"],
            ),
            expect_equal(
                "tools scope points at the tools group",
                VIS_TOOLS,
                ["//tools/visibility:tools_repo"],
            ),
            expect_equal(
                "tools public scope points at the tools public group",
                VIS_TOOLS_PUBLIC,
                ["//tools/visibility:tools_public"],
            ),
            expect_equal(
                "tools cli test scope points at the cli test group",
                VIS_TOOLS_CLI_TEST,
                ["//tools/visibility:tools_cli_test"],
            ),
            expect_equal(
                "dx roots scope points at the dx roots group",
                VIS_DX_ROOTS,
                ["//tools/visibility:dx_roots"],
            ),
            expect_equal(
                "dx commands scope points at the dx commands group",
                VIS_DX_COMMANDS,
                ["//tools/visibility:dx_commands"],
            ),
        ],
    )
