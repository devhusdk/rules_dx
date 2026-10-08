"""Unit and analysis tests for the npm pack feed publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":npm.bzl", "npm_filenames", "npm_package_error", "npm_registry_error", "npm_tag_error")

def npm_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "npm_filenames pins the tarball and feed names",
                npm_filenames("npm_demo"),
                ("npm_demo.tgz", "npm_demo.feed.json"),
            ),
            expect_equal(
                "npm_filenames derives names per instance",
                npm_filenames("release"),
                ("release.tgz", "release.feed.json"),
            ),
            expect_equal(
                "npm_package_error accepts a plain package name",
                npm_package_error("npm-demo"),
                "",
            ),
            expect_equal(
                "npm_package_error accepts a scoped package name",
                npm_package_error("@scope/npm-demo"),
                "",
            ),
            expect_equal(
                "npm_package_error rejects an empty package name",
                npm_package_error(""),
                "npm_deploy: invalid package '': want a non-empty " +
                "package name (for example 'npm-demo')",
            ),
            expect_equal(
                "npm_package_error rejects shell-unsafe package characters",
                npm_package_error("bad;pkg"),
                "npm_deploy: invalid package 'bad;pkg': want only " +
                "[A-Za-z0-9@/_.-] so the package name embeds safely " +
                "in the deploy program",
            ),
            expect_equal(
                "npm_tag_error accepts the latest placeholder tag",
                npm_tag_error("latest"),
                "",
            ),
            expect_equal(
                "npm_tag_error accepts dotted prerelease tags",
                npm_tag_error("next"),
                "",
            ),
            expect_equal(
                "npm_tag_error rejects an empty tag",
                npm_tag_error(""),
                "npm_deploy: invalid tag '': want a non-empty tag " +
                "(for example 'latest')",
            ),
            expect_equal(
                "npm_tag_error rejects shell-unsafe tag characters",
                npm_tag_error("v1.0;evil"),
                "npm_deploy: invalid tag 'v1.0;evil': want only " +
                "[A-Za-z0-9._-] so the tag embeds safely in the deploy program",
            ),
            expect_equal(
                "npm_registry_error accepts the default registry",
                npm_registry_error("https://registry.npmjs.org"),
                "",
            ),
            expect_equal(
                "npm_registry_error rejects a non-https registry",
                npm_registry_error("http://registry.npmjs.org"),
                "npm_deploy: invalid registry 'http://registry.npmjs.org': " +
                "want an https:// URL (for example 'https://registry.npmjs.org')",
            ),
            expect_equal(
                "npm_registry_error rejects an empty registry",
                npm_registry_error(""),
                "npm_deploy: invalid registry '': want an https:// URL " +
                "(for example 'https://registry.npmjs.org')",
            ),
        ],
    )

EXPECTED_NPM_DEFAULT_OBSERVATIONS = """subject //deploy/rules:npm_demo
file npm_demo
field app=
field artifacts=
field profile=release
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:npm_demo
aspect_field transitive_count=0"""

EXPECTED_NPM_DEBUG_OBSERVATIONS = """subject //deploy/rules:npm_demo_debug
file npm_demo_debug
field app=
field artifacts=
field profile=debug
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:npm_demo_debug
aspect_field transitive_count=0"""

def npm_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":npm_demo"],
        expected_observations = EXPECTED_NPM_DEFAULT_OBSERVATIONS,
    )

def npm_debug_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":npm_demo_debug"],
        expected_observations = EXPECTED_NPM_DEBUG_OBSERVATIONS,
    )
