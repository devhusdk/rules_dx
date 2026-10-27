"""Unit tests for SBOM + provenance generation."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":sbom.bzl", "SBOM_BUILDER_DRY_RUN", "SBOM_BUILDER_RELEASE", "sbom_builder_error", "sbom_filenames")

def sbom_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "sbom_filenames pins spdx and provenance names",
                sbom_filenames("release"),
                ("release.spdx.json", "release.provenance.json"),
            ),
            expect_equal(
                "sbom_filenames derives names per instance",
                sbom_filenames("dx_standalone"),
                ("dx_standalone.spdx.json", "dx_standalone.provenance.json"),
            ),
            expect_equal(
                "sbom_builder_error accepts the dry-run builder",
                sbom_builder_error(SBOM_BUILDER_DRY_RUN),
                "",
            ),
            expect_equal(
                "sbom_builder_error accepts the release builder",
                sbom_builder_error(SBOM_BUILDER_RELEASE),
                "",
            ),
            expect_equal(
                "sbom_builder_error rejects a forged builder",
                sbom_builder_error("https://example.com/builder"),
                "sbom_release: invalid builder 'https://example.com/builder': want '" + SBOM_BUILDER_DRY_RUN + "' (dry-run demo only) or '" + SBOM_BUILDER_RELEASE + "' (owner-approved release)",
            ),
        ],
    )

def sbom_file_tests(name, spdx, want):
    """Checks that an SPDX output contains the wanted text."""
    starlark_test(
        name = name,
        mode = "execution",
        file_checks = {spdx: want},
    )
