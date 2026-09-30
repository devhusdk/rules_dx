"""Unit tests for vendored offline bundle manifests."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":offline.bzl", "offline_manifest_name", "offline_set_error", "offline_srcs_error")

def offline_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "offline_manifest_name pins manifest and checksum names",
                offline_manifest_name("bundle"),
                "bundle.SHA256SUMS",
            ),
            expect_equal(
                "offline_manifest_name derives names per instance",
                offline_manifest_name("offline_demo"),
                "offline_demo.SHA256SUMS",
            ),
            expect_equal(
                "offline_set_error accepts every vendored set",
                [offline_set_error(set) for set in ["cargo", "npm", "maven", "nuget", "go", "rubygems"]],
                ["", "", "", "", "", ""],
            ),
            expect_equal(
                "offline_set_error rejects an unknown set",
                offline_set_error("pypi"),
                "offline_bundle: invalid advisory set 'pypi': want one of cargo, npm, maven, nuget, go, rubygems",
            ),
            expect_equal(
                "offline_srcs_error rejects an empty bundle",
                offline_srcs_error([]),
                "offline_bundle: need at least one bundle file",
            ),
            expect_equal(
                "offline_srcs_error accepts a nonempty bundle",
                offline_srcs_error(["//deploy/offline:demo"]),
                "",
            ),
        ],
    )

def offline_file_tests(name, manifest):
    starlark_test(
        name = name,
        mode = "execution",
        file_checks = {
            manifest: "demo-bazelisk-linux-amd64",
        },
    )
