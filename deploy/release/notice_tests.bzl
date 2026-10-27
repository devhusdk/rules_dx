"""Unit tests for aggregated NOTICE bundling."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":notice.bzl", "notice_filenames", "notice_manifest_error", "notice_root_error")

def notice_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "notice_filenames pins NOTICE and checksum names",
                notice_filenames("release"),
                ("release.NOTICE", "release.NOTICE.sha256"),
            ),
            expect_equal(
                "notice_filenames derives names per instance",
                notice_filenames("notice_demo"),
                ("notice_demo.NOTICE", "notice_demo.NOTICE.sha256"),
            ),
            expect_equal(
                "notice_root_error accepts a distributed-tier root label",
                notice_root_error("//deploy/release:notice_demo"),
                "",
            ),
            expect_equal(
                "notice_root_error rejects an empty root",
                notice_root_error(""),
                "notice_bundle: invalid root '': want a non-empty distributed-tier root label",
            ),
            expect_equal(
                "notice_manifest_error rejects a missing manifest",
                notice_manifest_error(None),
                "notice_bundle: invalid manifest 'None': want the audited inventory manifest label",
            ),
        ],
    )

def notice_file_tests(name, notice, want = "NOTICE for\nFixture license words for demo-lib-a\nFixture license words for demo-lib-b"):
    starlark_test(
        name = name,
        mode = "execution",
        file_checks = {
            notice: want,
        },
    )
