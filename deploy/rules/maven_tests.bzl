"""Unit and analysis tests for the Maven Central publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":maven_validate.bzl", "maven_artifact_error", "maven_group_error", "maven_jar_error", "maven_javadoc_error", "maven_pom_error", "maven_sources_error", "maven_version_error")

def maven_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "maven_group_error accepts the demo group",
                maven_group_error("com.example"),
                "",
            ),
            expect_equal(
                "maven_group_error rejects an empty group",
                maven_group_error(""),
                "maven_deploy: invalid group '': want a non-empty " +
                "groupId (for example 'com.example')",
            ),
            expect_equal(
                "maven_group_error rejects group characters",
                maven_group_error("com;curl evil"),
                "maven_deploy: invalid group 'com;curl evil': want " +
                "only [A-Za-z0-9._-]",
            ),
            expect_equal(
                "maven_group_error rejects a leading dot",
                maven_group_error(".example"),
                "maven_deploy: invalid group '.example': want non-empty " +
                "dot-separated segments (no leading, trailing, or repeated dots)",
            ),
            expect_equal(
                "maven_group_error rejects a trailing dot",
                maven_group_error("com."),
                "maven_deploy: invalid group 'com.': want non-empty " +
                "dot-separated segments (no leading, trailing, or repeated dots)",
            ),
            expect_equal(
                "maven_group_error rejects an empty segment",
                maven_group_error("com..example"),
                "maven_deploy: invalid group 'com..example': want non-empty " +
                "dot-separated segments (no leading, trailing, or repeated dots)",
            ),
            expect_equal(
                "maven_artifact_error rejects parent and current components",
                [maven_artifact_error(".") != "", maven_artifact_error("..") != ""],
                [True, True],
            ),
            expect_equal(
                "maven_version_error rejects parent and current components",
                [maven_version_error(".") != "", maven_version_error("..") != ""],
                [True, True],
            ),
            expect_equal(
                "maven_version_error accepts qualified versions",
                maven_version_error("1.0.0-SNAPSHOT+build.5"),
                "",
            ),
            expect_equal(
                "maven_artifact_error accepts the demo artifact",
                maven_artifact_error("maven_demo"),
                "",
            ),
            expect_equal(
                "maven_artifact_error accepts dashed artifacts",
                maven_artifact_error("dx-plugin"),
                "",
            ),
            expect_equal(
                "maven_artifact_error rejects an empty artifact",
                maven_artifact_error(""),
                "maven_deploy: invalid artifact '': want a non-empty " +
                "artifactId (for example 'maven_demo')",
            ),
            expect_equal(
                "maven_artifact_error rejects artifact characters",
                maven_artifact_error("dx;curl evil"),
                "maven_deploy: invalid artifact 'dx;curl evil': want " +
                "only [A-Za-z0-9._-]",
            ),
            expect_equal(
                "maven_version_error accepts the demo version",
                maven_version_error("0.0.0"),
                "",
            ),
            expect_equal(
                "maven_version_error accepts prerelease versions",
                maven_version_error("1.2.3-alpha.1"),
                "",
            ),
            expect_equal(
                "maven_version_error rejects an empty version",
                maven_version_error(""),
                "maven_deploy: invalid version '': want a non-empty " +
                "version (for example '0.0.0')",
            ),
            expect_equal(
                "maven_version_error rejects version characters",
                maven_version_error("1.0;curl evil"),
                "maven_deploy: invalid version '1.0;curl evil': want " +
                "only [A-Za-z0-9._-+]",
            ),
            expect_equal(
                "maven_jar_error accepts a pinned jar filename",
                maven_jar_error("maven_demo-0.0.0.jar"),
                "",
            ),
            expect_equal(
                "maven_jar_error rejects a non-jar filename",
                maven_jar_error("maven_demo-0.0.0.pom"),
                "maven_deploy: invalid jar 'maven_demo-0.0.0.pom': want a " +
                "filename ending in .jar",
            ),
            expect_equal(
                "maven_sources_error accepts a jar filename",
                maven_sources_error("maven_demo-sources.jar"),
                "",
            ),
            expect_equal(
                "maven_sources_error rejects a non-jar filename",
                maven_sources_error("sources.zip"),
                "maven_deploy: invalid sources jar 'sources.zip': want a " +
                "filename ending in .jar",
            ),
            expect_equal(
                "maven_javadoc_error accepts a jar filename",
                maven_javadoc_error("maven_demo-javadoc.jar"),
                "",
            ),
            expect_equal(
                "maven_pom_error accepts a pinned pom filename",
                maven_pom_error("maven_demo-0.0.0.pom"),
                "",
            ),
            expect_equal(
                "maven_pom_error rejects a non-pom filename",
                maven_pom_error("maven_demo-0.0.0.jar"),
                "maven_deploy: invalid pom 'maven_demo-0.0.0.jar': want a " +
                "filename ending in .pom",
            ),
        ],
    )
