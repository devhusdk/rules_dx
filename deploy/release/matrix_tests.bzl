"""Unit tests for the release matrix."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":matrix.bzl", "release_matrix_error", "release_matrix_names", "release_matrix_status", "release_matrix_unqualified")

def matrix_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "matrix names pin the five-cell order, seed first",
                release_matrix_names(),
                ["dx-linux-x86_64", "dx-linux-arm64", "dx-macos-arm64", "dx-windows-x86_64", "dx-windows-arm64"],
            ),
            expect_equal(
                "matrix seed cell is qualified-built-here",
                release_matrix_status("dx-linux-x86_64"),
                "qualified-seed-built-here",
            ),
            expect_equal(
                "matrix linux arm64 is qualified with host evidence",
                release_matrix_status("dx-linux-arm64"),
                "qualified-host-evidence",
            ),
            expect_equal(
                "matrix macOS arm64 is qualified with host evidence",
                release_matrix_status("dx-macos-arm64"),
                "qualified-host-evidence",
            ),
            expect_equal(
                "matrix windows x86_64 is qualified with host evidence",
                release_matrix_status("dx-windows-x86_64"),
                "qualified-host-evidence",
            ),
            expect_equal(
                "matrix windows arm64 awaits native evidence",
                release_matrix_status("dx-windows-arm64"),
                "unqualified-missing-native-evidence",
            ),
            expect_equal(
                "matrix unknown name reports empty status",
                release_matrix_status("dx-plan9-mips"),
                "",
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
                "matrix unqualified names the windows arm64 gap",
                release_matrix_unqualified(),
                ["dx-windows-arm64"],
            ),
        ],
    )
