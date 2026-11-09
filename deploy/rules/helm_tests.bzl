"""Unit tests for the Helm chart publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":validate_helm.bzl", "helm_version_error")

def helm_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "helm_version_error accepts a release version",
                helm_version_error("1.2.3-rc.1"),
                "",
            ),
            expect_equal(
                "helm_version_error rejects an empty version",
                helm_version_error(""),
                "helm_chart_deploy: invalid version '': want the Chart.yaml " +
                "version (for example '1.2.3')",
            ),
            expect_equal(
                "helm_version_error rejects version characters",
                helm_version_error("1.0;x"),
                "helm_chart_deploy: invalid version '1.0;x': want only " +
                "[A-Za-z0-9._+-]",
            ),
        ],
    )
