"""Unit and analysis tests for the Maven Central publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":maven.bzl", "MAVEN_DEFAULT_REPOSITORY_URL", "maven_artifact_error", "maven_group_error", "maven_jar_error", "maven_pom_error", "maven_repository_error", "maven_version_error")

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
                "maven_group_error rejects launcher-unsafe group characters",
                maven_group_error("com;curl evil"),
                "maven_deploy: invalid group 'com;curl evil': want " +
                "only [A-Za-z0-9._-] so the group embeds safely in the " +
                "deploy launcher",
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
                "maven_artifact_error rejects launcher-unsafe artifact characters",
                maven_artifact_error("dx;curl evil"),
                "maven_deploy: invalid artifact 'dx;curl evil': want " +
                "only [A-Za-z0-9._-] so the artifact embeds safely in " +
                "the deploy launcher",
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
                "maven_version_error rejects launcher-unsafe version characters",
                maven_version_error("1.0;curl evil"),
                "maven_deploy: invalid version '1.0;curl evil': want " +
                "only [A-Za-z0-9._-+] so the version embeds safely in " +
                "the deploy launcher",
            ),
            expect_equal(
                "maven_repository_error accepts the default Publisher URL",
                maven_repository_error(MAVEN_DEFAULT_REPOSITORY_URL),
                "",
            ),
            expect_equal(
                "maven_repository_error rejects the retired OSSRH host",
                maven_repository_error("https://oss.sonatype.org/service/local/staging/deploy/maven2/"),
                "maven_deploy: invalid repository_url 'https://oss.sonatype.org/service/local/staging/deploy/maven2/': " +
                "oss.sonatype.org is retired OSSRH infrastructure (use '" +
                MAVEN_DEFAULT_REPOSITORY_URL + "')",
            ),
            expect_equal(
                "maven_repository_error accepts a mixed-case Publisher host",
                maven_repository_error("https://Central.Sonatype.com/api/v1/publisher/"),
                "",
            ),
            expect_equal(
                "maven_repository_error rejects an unsupported host",
                maven_repository_error("https://repo.example.com/maven2/"),
                "maven_deploy: invalid repository_url 'https://repo.example.com/maven2/': " +
                "want the Central Portal Publisher API '" + MAVEN_DEFAULT_REPOSITORY_URL + "'",
            ),
            expect_equal(
                "maven_repository_error rejects the OSSRH staging API host",
                maven_repository_error("https://ossrh-staging-api.central.sonatype.com/service/local/staging/deploy/maven2/"),
                "maven_deploy: invalid repository_url " +
                "'https://ossrh-staging-api.central.sonatype.com/service/local/staging/deploy/maven2/': " +
                "want the Central Portal Publisher API '" + MAVEN_DEFAULT_REPOSITORY_URL + "'",
            ),
            expect_equal(
                "maven_repository_error rejects a wrong Publisher path",
                maven_repository_error("https://central.sonatype.com/api/v1/"),
                "maven_deploy: invalid repository_url 'https://central.sonatype.com/api/v1/': " +
                "want the Central Portal Publisher API '" + MAVEN_DEFAULT_REPOSITORY_URL + "'",
            ),
            expect_equal(
                "maven_repository_error rejects plaintext URLs",
                maven_repository_error("http://example.com/maven2/"),
                "maven_deploy: invalid repository_url 'http://example.com/maven2/': " +
                "want an https URL so uploads never go over plaintext",
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

EXPECTED_MAVEN_DEFAULT_OBSERVATIONS = """subject //deploy/rules:maven_demo
file maven_demo
field app=
field artifacts=
field profile=release
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:maven_demo
aspect_field transitive_count=0"""

EXPECTED_MAVEN_DEBUG_OBSERVATIONS = """subject //deploy/rules:maven_demo_debug
file maven_demo_debug
field app=
field artifacts=
field profile=debug
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:maven_demo_debug
aspect_field transitive_count=0"""

def maven_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":maven_demo"],
        expected_observations = EXPECTED_MAVEN_DEFAULT_OBSERVATIONS,
    )

def maven_debug_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":maven_demo_debug"],
        expected_observations = EXPECTED_MAVEN_DEBUG_OBSERVATIONS,
    )
