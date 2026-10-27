"""Unit and analysis tests for the OCI publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":oci.bzl", "OCI_DEFAULT_REGISTRY", "oci_platform_error", "oci_registry_error", "oci_repository_error", "oci_tag_error", "oci_tar_error")

def oci_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "oci_tag_error accepts the latest placeholder tag",
                oci_tag_error("latest"),
                "",
            ),
            expect_equal(
                "oci_tag_error accepts dotted tags",
                oci_tag_error("0.0.0-dryrun"),
                "",
            ),
            expect_equal(
                "oci_tag_error rejects an empty tag",
                oci_tag_error(""),
                "oci_deploy: invalid tag '': want a non-empty tag " +
                "(for example 'latest')",
            ),
            expect_equal(
                "oci_tag_error rejects launcher-unsafe tag characters",
                oci_tag_error("v1.0;curl evil"),
                "oci_deploy: invalid tag 'v1.0;curl evil': want " +
                "only [A-Za-z0-9._-] so the tag embeds safely in the deploy launcher",
            ),
            expect_equal(
                "oci_registry_error accepts the default registry",
                oci_registry_error(OCI_DEFAULT_REGISTRY),
                "",
            ),
            expect_equal(
                "oci_registry_error accepts a registry path",
                oci_registry_error("ghcr.io/example"),
                "",
            ),
            expect_equal(
                "oci_registry_error rejects a URL scheme",
                oci_registry_error("https://ghcr.io"),
                "oci_deploy: invalid registry 'https://ghcr.io': want a " +
                "host plus optional path, never a URL scheme",
            ),
            expect_equal(
                "oci_registry_error rejects an empty registry",
                oci_registry_error(""),
                "oci_deploy: invalid registry '': want a non-empty " +
                "registry (for example '" + OCI_DEFAULT_REGISTRY + "')",
            ),
            expect_equal(
                "oci_repository_error accepts the demo repository",
                oci_repository_error("oci_demo"),
                "",
            ),
            expect_equal(
                "oci_repository_error accepts a namespaced repository",
                oci_repository_error("example/oci_demo"),
                "",
            ),
            expect_equal(
                "oci_repository_error rejects an empty repository",
                oci_repository_error(""),
                "oci_deploy: invalid repository '': want a non-empty " +
                "repository (for example 'oci_demo')",
            ),
            expect_equal(
                "oci_tag_error accepts v1.2.3",
                oci_tag_error("v1.2.3"),
                "",
            ),
            expect_equal(
                "oci_tag_error accepts latest",
                oci_tag_error("latest"),
                "",
            ),
            expect_equal(
                "oci_tag_error accepts _x",
                oci_tag_error("_x"),
                "",
            ),
            expect_equal(
                "oci_tag_error accepts 128 chars",
                oci_tag_error("a" * 128),
                "",
            ),
            expect_equal(
                "oci_tag_error rejects leading -",
                oci_tag_error("-rc1"),
                "oci_deploy: invalid tag '-rc1': want a tag starting with [A-Za-z0-9_]",
            ),
            expect_equal(
                "oci_tag_error rejects leading .",
                oci_tag_error(".x"),
                "oci_deploy: invalid tag '.x': want a tag starting with [A-Za-z0-9_]",
            ),
            expect_equal(
                "oci_tag_error rejects 129 chars",
                oci_tag_error("a" * 129),
                "oci_deploy: invalid tag '" + "a" * 129 + "': want at most 128 characters",
            ),
            expect_equal(
                "oci_repository_error accepts a__b",
                oci_repository_error("a__b"),
                "",
            ),
            expect_equal(
                "oci_repository_error accepts a--b",
                oci_repository_error("a--b"),
                "",
            ),
            expect_equal(
                "oci_repository_error accepts a.b-c_d",
                oci_repository_error("a.b-c_d"),
                "",
            ),
            expect_equal(
                "oci_repository_error accepts example/oci_demo",
                oci_repository_error("example/oci_demo"),
                "",
            ),
            expect_equal(
                "oci_repository_error accepts oci_demo",
                oci_repository_error("oci_demo"),
                "",
            ),
            expect_equal(
                "oci_repository_error hints lowercase and target name",
                oci_repository_error("MyApp", from_name = True),
                "oci_deploy: invalid repository 'MyApp': want lowercase (for example 'myapp'); " +
                "repository defaults to the target name, set repository = '...' explicitly",
            ),
            expect_equal(
                "oci_repository_error lowercase hint without target-name suffix",
                oci_repository_error("Example/app"),
                "oci_deploy: invalid repository 'Example/app': want lowercase (for example 'example/app')",
            ),
            expect_equal(
                "oci_repository_error rejects a..b",
                oci_repository_error("a..b"),
                "oci_deploy: invalid repository 'a..b': want separators to be '.', '_', '__' or dashes only",
            ),
            expect_equal(
                "oci_repository_error rejects a._b",
                oci_repository_error("a._b"),
                "oci_deploy: invalid repository 'a._b': want separators to be '.', '_', '__' or dashes only",
            ),
            expect_equal(
                "oci_repository_error rejects a___b",
                oci_repository_error("a___b"),
                "oci_deploy: invalid repository 'a___b': want separators to be '.', '_', '__' or dashes only",
            ),
            expect_equal(
                "oci_repository_error rejects -app",
                oci_repository_error("-app"),
                "oci_deploy: invalid repository '-app': want each path segment to start and end with [a-z0-9]",
            ),
            expect_equal(
                "oci_repository_error rejects app-",
                oci_repository_error("app-"),
                "oci_deploy: invalid repository 'app-': want each path segment to start and end with [a-z0-9]",
            ),
            expect_equal(
                "oci_repository_error rejects a/.b",
                oci_repository_error("a/.b"),
                "oci_deploy: invalid repository 'a/.b': want each path segment to start and end with [a-z0-9]",
            ),
            expect_equal(
                "oci_tar_error accepts a pinned tar filename",
                oci_tar_error("oci_demo.tar"),
                "",
            ),
            expect_equal(
                "oci_platform_error accepts linux/amd64",
                oci_platform_error("linux/amd64"),
                "",
            ),
            expect_equal(
                "oci_platform_error accepts linux/arm64",
                oci_platform_error("linux/arm64"),
                "",
            ),
            expect_equal(
                "oci_platform_error rejects an empty platform",
                oci_platform_error(""),
                "oci_deploy: invalid platform '': want one of " +
                "linux/amd64, linux/arm64",
            ),
            expect_equal(
                "oci_platform_error rejects windows",
                oci_platform_error("windows/amd64"),
                "oci_deploy: invalid platform 'windows/amd64': want one of " +
                "linux/amd64, linux/arm64",
            ),
            expect_equal(
                "oci_tar_error rejects a non-tar filename",
                oci_tar_error("oci_demo.zip"),
                "oci_deploy: invalid image_tar 'oci_demo.zip': want a " +
                "filename ending in .tar or .tar.gz",
            ),
        ],
    )
