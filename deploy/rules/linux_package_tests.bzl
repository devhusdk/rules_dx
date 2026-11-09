"""Unit tests for the Linux package deploy helpers."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":linux_package.bzl", "linux_package_filename")
load(":validate_linux_package.bzl", "linux_package_arch_error", "linux_package_dst_error", "linux_package_formats_error", "linux_package_name_error", "linux_package_version_error")

def linux_package_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "linux_package_arch_error accepts linux x86_64",
                linux_package_arch_error("linux", "x86_64"),
                "",
            ),
            expect_equal(
                "linux_package_arch_error accepts linux aarch64",
                linux_package_arch_error("linux", "aarch64"),
                "",
            ),
            expect_equal(
                "linux_package_arch_error rejects macos",
                linux_package_arch_error("macos", "aarch64"),
                "linux_package_deploy: unsupported os 'macos': want linux",
            ),
            expect_equal(
                "linux_package_arch_error rejects a linux riscv64",
                linux_package_arch_error("linux", "riscv64"),
                "linux_package_deploy: unsupported cpu 'riscv64': want one of aarch64, x86_64",
            ),
            expect_equal(
                "linux_package_name_error accepts a plain name",
                linux_package_name_error("demo-app"),
                "",
            ),
            expect_equal(
                "linux_package_name_error rejects unsafe characters",
                linux_package_name_error("a b"),
                "linux_package_deploy: invalid package name 'a b': want only [A-Za-z0-9._-]",
            ),
            expect_equal(
                "linux_package_formats_error accepts deb and rpm",
                linux_package_formats_error(["deb", "rpm"]),
                "",
            ),
            expect_equal(
                "linux_package_formats_error rejects unknown formats",
                linux_package_formats_error(["zip"]),
                "linux_package_deploy: invalid format 'zip': want one of [\"deb\", \"rpm\", \"apk\"]",
            ),
            expect_equal(
                "linux_package_formats_error rejects duplicates",
                linux_package_formats_error(["deb", "deb"]),
                "linux_package_deploy: duplicate format 'deb'",
            ),
            expect_equal(
                "linux_package_dst_error accepts an absolute path",
                linux_package_dst_error("/usr/bin/demo"),
                "",
            ),
            expect_equal(
                "linux_package_dst_error rejects a relative path",
                linux_package_dst_error("demo"),
                "linux_package_deploy: invalid install path 'demo': want an absolute file path " +
                "(for example '/usr/bin/demo')",
            ),
            expect_equal(
                "linux_package_filename joins name, version, arch and format",
                linux_package_filename("demo", "1.2.3", "amd64", "deb"),
                "demo_1.2.3_amd64.deb",
            ),
            expect_equal(
                "linux_package_version_error accepts 1.2.3",
                linux_package_version_error("1.2.3"),
                "",
            ),
            expect_equal(
                "linux_package_version_error accepts a prerelease with build metadata",
                linux_package_version_error("1.2.3-rc.1+b1"),
                "",
            ),
            expect_equal(
                "linux_package_version_error rejects an empty version",
                linux_package_version_error(""),
                "linux_package_deploy: invalid version '': want a non-empty version (for example '1.2.3')",
            ),
            expect_equal(
                "linux_package_version_error rejects a space",
                linux_package_version_error("a b"),
                "linux_package_deploy: invalid version 'a b': want a non-empty version (for example '1.2.3')",
            ),
            expect_equal(
                "linux_package_version_error rejects a leading v",
                linux_package_version_error("v1.2.3"),
                "linux_package_deploy: invalid version 'v1.2.3': want a non-empty version (for example '1.2.3')",
            ),
        ],
    )
