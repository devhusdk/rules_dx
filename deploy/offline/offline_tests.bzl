"""Unit tests for vendored offline bundle manifests."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":offline.bzl", "offline_basenames_error", "offline_bundle_layout", "offline_launcher_error", "offline_set_error", "offline_srcs_error")

def offline_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "offline_launcher_error accepts every host launcher",
                [offline_launcher_error(l) for l in [
                    "bazelisk-darwin-amd64",
                    "bazelisk-darwin-arm64",
                    "bazelisk-linux-amd64",
                    "bazelisk-linux-arm64",
                    "bazelisk-windows-amd64.exe",
                ]],
                ["", "", "", "", ""],
            ),
            expect_equal(
                "offline_launcher_error rejects a name bootstrap would not pick",
                offline_launcher_error("demo-bazelisk-linux-amd64"),
                "offline_bundle: invalid launcher 'demo-bazelisk-linux-amd64': want one of " +
                "bazelisk-darwin-amd64, bazelisk-darwin-arm64, bazelisk-linux-amd64, " +
                "bazelisk-linux-arm64, bazelisk-windows-amd64.exe",
            ),
            expect_equal(
                "offline_bundle_layout lists the tree bootstrap reads, sorted",
                offline_bundle_layout(
                    "b",
                    {"npm": ":n.json", "cargo": "//x:c.json"},
                    [":d/bazelisk-linux-arm64", ":d/bazelisk-linux-amd64"],
                ),
                [
                    ["advisory", "cargo.json", "//x:c.json"],
                    ["advisory", "npm.json", ":n.json"],
                    ["bazelisk", "bazelisk-linux-amd64", ":d/bazelisk-linux-amd64"],
                    ["bazelisk", "bazelisk-linux-arm64", ":d/bazelisk-linux-arm64"],
                ],
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
            expect_equal(
                "offline_basenames_error accepts unique basenames",
                offline_basenames_error([":demo/bazelisk-linux-arm64", ":demo/bazelisk-linux-amd64"]),
                "",
            ),
            expect_equal(
                "offline_basenames_error rejects one name in two packages",
                offline_basenames_error(["//a:LICENSE", "//b:LICENSE"]),
                "offline_bundle: duplicate bundle basename 'LICENSE': //a:LICENSE and //b:LICENSE",
            ),
            expect_equal(
                "offline_basenames_error rejects one name in two subdirectories",
                offline_basenames_error([":a/LICENSE", "//x:b/LICENSE"]),
                "offline_bundle: duplicate bundle basename 'LICENSE': :a/LICENSE and //x:b/LICENSE",
            ),
            expect_equal(
                "offline_basenames_error rejects one name across repos",
                offline_basenames_error(["//p:LICENSE", "@r//q:LICENSE"]),
                "offline_bundle: duplicate bundle basename 'LICENSE': //p:LICENSE and @r//q:LICENSE",
            ),
            expect_equal(
                "offline_basenames_error accepts bare names",
                offline_basenames_error(["LICENSE", "NOTICE"]),
                "",
            ),
        ],
    )

def offline_file_tests(name, advisory, bazelisk):
    starlark_test(
        name = name,
        mode = "execution",
        file_checks = {
            advisory: "cargo.json",
            bazelisk: "bazelisk-linux-amd64\nbazelisk-windows-amd64.exe",
        },
    )
