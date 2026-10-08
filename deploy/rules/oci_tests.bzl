"""Unit and analysis tests for the OCI publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":oci.bzl", "OCI_DEFAULT_REGISTRY", "oci_registry_error", "oci_repository_error", "oci_schema_error", "oci_tag_error", "oci_tar_error")

def oci_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "oci_schema_error accepts the v1 launcher-safe charset",
                oci_schema_error(),
                "",
            ),
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
                "oci_tar_error accepts a pinned tar filename",
                oci_tar_error("oci_demo.tar"),
                "",
            ),
            expect_equal(
                "oci_tar_error rejects a non-tar filename",
                oci_tar_error("oci_demo.zip"),
                "oci_deploy: invalid image_tar 'oci_demo.zip': want a " +
                "filename ending in .tar or .tar.gz",
            ),
        ],
    )

EXPECTED_OCI_DEFAULT_OBSERVATIONS = """subject //deploy/rules:oci_demo
file oci_demo
field app=
field artifacts=
field profile=release
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:oci_demo
aspect_field transitive_count=0"""

EXPECTED_OCI_DEBUG_OBSERVATIONS = """subject //deploy/rules:oci_demo_debug
file oci_demo_debug
field app=
field artifacts=
field profile=debug
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:oci_demo_debug
aspect_field transitive_count=0"""

def oci_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":oci_demo"],
        expected_observations = EXPECTED_OCI_DEFAULT_OBSERVATIONS,
    )

def oci_debug_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":oci_demo_debug"],
        expected_observations = EXPECTED_OCI_DEBUG_OBSERVATIONS,
    )
