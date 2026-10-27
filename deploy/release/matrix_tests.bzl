"""Unit tests for the release matrix."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":matrix.bzl", "release_cell_archive", "release_matrix_cell", "release_matrix_error", "release_matrix_json", "release_matrix_names")

def _field(field):
    """Returns one field of every cell in matrix order."""
    return [release_matrix_cell(name)[field] for name in release_matrix_names()]

def matrix_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "matrix names pin the five-cell order",
                release_matrix_names(),
                ["dx-linux-x86_64", "dx-linux-arm64", "dx-macos-arm64", "dx-windows-x86_64", "dx-windows-arm64"],
            ),
            expect_equal(
                "matrix formats are tar.gz on unix and zip on windows",
                _field("format"),
                ["tar.gz", "tar.gz", "tar.gz", "zip", "zip"],
            ),
            expect_equal(
                "matrix exes carry .exe on windows only",
                _field("exe"),
                ["dx", "dx", "dx", "dx.exe", "dx.exe"],
            ),
            expect_equal(
                "matrix maps arm64 to the aarch64 machine",
                _field("machine"),
                ["x86_64", "aarch64", "aarch64", "x86_64", "aarch64"],
            ),
            expect_equal(
                "matrix constraints pin os and cpu",
                release_matrix_cell("dx-macos-arm64")["constraints"],
                ["@platforms//os:macos", "@platforms//cpu:aarch64"],
            ),
            expect_equal(
                "matrix archive names follow the cell format",
                [release_cell_archive(release_matrix_cell(n)) for n in release_matrix_names()],
                ["dx-linux-x86_64.tar.gz", "dx-linux-arm64.tar.gz", "dx-macos-arm64.tar.gz", "dx-windows-x86_64.zip", "dx-windows-arm64.zip"],
            ),
            expect_equal(
                "matrix unknown name has no cell",
                release_matrix_cell("dx-plan9-mips"),
                None,
            ),
            expect_equal(
                "matrix error accepts a known cell",
                release_matrix_error("dx-linux-x86_64"),
                "",
            ),
            expect_equal(
                "matrix error rejects an unknown cell",
                release_matrix_error("dx-plan9-mips"),
                "release_matrix: unknown artifact 'dx-plan9-mips': want one of dx-linux-x86_64, dx-linux-arm64, dx-macos-arm64, dx-windows-x86_64, dx-windows-arm64",
            ),
            expect_equal(
                "matrix json is the exact release-matrix/v1 text",
                release_matrix_json(),
                (
                    "{\n" +
                    "  \"cells\": [\n" +
                    "    {\n" +
                    "      \"arch\": \"x86_64\",\n" +
                    "      \"exe\": \"dx\",\n" +
                    "      \"format\": \"tar.gz\",\n" +
                    "      \"machine\": \"x86_64\",\n" +
                    "      \"name\": \"dx-linux-x86_64\",\n" +
                    "      \"os\": \"linux\",\n" +
                    "      \"runtime\": []\n" +
                    "    },\n" +
                    "    {\n" +
                    "      \"arch\": \"arm64\",\n" +
                    "      \"exe\": \"dx\",\n" +
                    "      \"format\": \"tar.gz\",\n" +
                    "      \"machine\": \"aarch64\",\n" +
                    "      \"name\": \"dx-linux-arm64\",\n" +
                    "      \"os\": \"linux\",\n" +
                    "      \"runtime\": []\n" +
                    "    },\n" +
                    "    {\n" +
                    "      \"arch\": \"arm64\",\n" +
                    "      \"exe\": \"dx\",\n" +
                    "      \"format\": \"tar.gz\",\n" +
                    "      \"machine\": \"aarch64\",\n" +
                    "      \"name\": \"dx-macos-arm64\",\n" +
                    "      \"os\": \"macos\",\n" +
                    "      \"runtime\": []\n" +
                    "    },\n" +
                    "    {\n" +
                    "      \"arch\": \"x86_64\",\n" +
                    "      \"exe\": \"dx.exe\",\n" +
                    "      \"format\": \"zip\",\n" +
                    "      \"machine\": \"x86_64\",\n" +
                    "      \"name\": \"dx-windows-x86_64\",\n" +
                    "      \"os\": \"windows\",\n" +
                    "      \"runtime\": []\n" +
                    "    },\n" +
                    "    {\n" +
                    "      \"arch\": \"arm64\",\n" +
                    "      \"exe\": \"dx.exe\",\n" +
                    "      \"format\": \"zip\",\n" +
                    "      \"machine\": \"aarch64\",\n" +
                    "      \"name\": \"dx-windows-arm64\",\n" +
                    "      \"os\": \"windows\",\n" +
                    "      \"runtime\": []\n" +
                    "    }\n" +
                    "  ],\n" +
                    "  \"schema\": \"release-matrix/v1\"\n" +
                    "}\n"
                ),
            ),
        ],
    )
