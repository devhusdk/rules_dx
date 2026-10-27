"""Unit tests for vendored offline bundle manifests."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":offline.bzl", "offline_basenames_error", "offline_bundle_layout", "offline_launcher_error", "offline_meta_error", "offline_set_error", "offline_srcs_error")

def offline_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "offline_launcher_error accepts every host launcher",
                [offline_launcher_error(launcher) for launcher in [
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
                    {"npm": ":n.meta.json", "cargo": "//x:c.meta.json"},
                ),
                [
                    ["advisory", "cargo.json", "//x:c.json"],
                    ["advisory", "cargo.meta.json", "//x:c.meta.json"],
                    ["advisory", "npm.json", ":n.json"],
                    ["advisory", "npm.meta.json", ":n.meta.json"],
                    ["bazelisk", "bazelisk-linux-amd64", ":d/bazelisk-linux-amd64"],
                    ["bazelisk", "bazelisk-linux-arm64", ":d/bazelisk-linux-arm64"],
                ],
            ),
            expect_equal(
                "offline_meta_error accepts one identity file per snapshot",
                offline_meta_error({"cargo": ":c.json"}, {"cargo": ":c.meta.json"}),
                "",
            ),
            expect_equal(
                "offline_meta_error rejects a snapshot with no identity file",
                offline_meta_error({"cargo": ":c.json", "npm": ":n.json"}, {"cargo": ":c.meta.json"}),
                "offline_bundle: advisory set 'npm' has no advisory_meta identity file: " +
                "add advisory_meta = {'npm': <npm.meta.json from advisory_prep>}",
            ),
            expect_equal(
                "offline_meta_error rejects an identity file with no snapshot",
                offline_meta_error({"cargo": ":c.json"}, {"cargo": ":c.meta.json", "npm": ":n.meta.json"}),
                "offline_bundle: advisory_meta set 'npm' has no snapshot in advisory",
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
                "offline_basenames_error rejects names that sanitize to one stem",
                offline_basenames_error(["//x:a.b", "//y:a_b"]),
                "offline_bundle: bundle basenames 'a.b' and 'a_b' both make target stem 'a_b': //x:a.b and //y:a_b",
            ),
            expect_equal(
                "offline_basenames_error rejects sanitized clash in reverse order",
                offline_basenames_error([":a_b", ":d/a.b"]),
                "offline_bundle: bundle basenames 'a_b' and 'a.b' both make target stem 'a_b': :a_b and :d/a.b",
            ),
            expect_equal(
                "offline_basenames_error accepts names whose stems differ",
                offline_basenames_error(["a.b", "a-b"]),
                "",
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
            advisory: "cargo.json\ncargo.meta.json",
            bazelisk: "bazelisk-linux-amd64\nbazelisk-windows-amd64.exe",
        },
    )
