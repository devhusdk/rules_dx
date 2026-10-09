"""Unit and analysis tests for the archive releaser."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(
    ":archive.bzl",
    "archive_filenames",
    "consumer_identity_error",
    "consumer_identity_text",
    "consumer_resource_member",
)

def archive_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "archive_filenames pins the tarball and checksum names",
                archive_filenames("release_demo"),
                ("release_demo.tar.gz", "release_demo.tar.gz.sha256"),
            ),
            expect_equal(
                "archive_filenames derives names per instance",
                archive_filenames("release"),
                ("release.tar.gz", "release.tar.gz.sha256"),
            ),
        ],
    )

EXPECTED_ARCHIVE_DEFAULT_OBSERVATIONS = """subject //deploy/rules:release_demo
file release_demo
field app=//deploy/rules:deploy_program
field artifacts=
field profile=release
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:release_demo
aspect_field transitive_count=0"""

EXPECTED_ARCHIVE_DEBUG_OBSERVATIONS = """subject //deploy/rules:release_demo_debug
file release_demo_debug
field app=//deploy/rules:deploy_program
field artifacts=
field profile=debug
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:release_demo_debug
aspect_field transitive_count=0"""

def archive_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":release_demo"],
        expected_observations = EXPECTED_ARCHIVE_DEFAULT_OBSERVATIONS,
    )

def archive_debug_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":release_demo_debug"],
        expected_observations = EXPECTED_ARCHIVE_DEBUG_OBSERVATIONS,
    )

def consumer_archive_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "consumer_identity_error accepts the Linux dynamic record",
                consumer_identity_error("x86_64", "linux", "dynamic", "glibc-2.17"),
                "",
            ),
            expect_equal(
                "consumer_identity_error accepts the macOS static record",
                consumer_identity_error("aarch64", "macos", "static", "macos-13"),
                "",
            ),
            expect_equal(
                "consumer_identity_error rejects an unknown arch",
                consumer_identity_error("sparc", "linux", "dynamic", "glibc-2.17"),
                "consumer_archive: unsupported arch 'sparc': want one of x86_64, aarch64",
            ),
            expect_equal(
                "consumer_identity_error rejects an unknown os",
                consumer_identity_error("x86_64", "solaris", "dynamic", "glibc-2.17"),
                "consumer_archive: unsupported os 'solaris': want one of linux, macos, windows",
            ),
            expect_equal(
                "consumer_identity_error rejects an unknown linkage",
                consumer_identity_error("x86_64", "linux", "shared", "glibc-2.17"),
                "consumer_archive: unsupported linkage 'shared': want one of static, dynamic",
            ),
            expect_equal(
                "consumer_identity_error rejects an empty min_runtime",
                consumer_identity_error("x86_64", "linux", "dynamic", ""),
                "consumer_archive: min_runtime must be a non-empty requirement",
            ),
            expect_equal(
                "consumer_identity_text renders the pinned manifest",
                consumer_identity_text("demo", "x86_64", "linux", "dynamic", "glibc-2.17"),
                "name: demo\narch: x86_64\nos: linux\nlinkage: dynamic\nmin_runtime: glibc-2.17\n",
            ),
            expect_equal(
                "consumer_resource_member maps a top-level staged file",
                consumer_resource_member(
                    "consumer_res",
                    "deploy/tests/fixtures/consumer_archive/consumer_res/banner.txt",
                ),
                "resources/banner.txt",
            ),
            expect_equal(
                "consumer_resource_member maps a nested staged file",
                consumer_resource_member(
                    "consumer_res",
                    "deploy/tests/fixtures/consumer_archive/consumer_res/generated/sprite.bin",
                ),
                "resources/generated/sprite.bin",
            ),
            expect_equal(
                "consumer_resource_member rejects a foreign short_path",
                consumer_resource_member("consumer_res", "deploy/rules/deploy_app"),
                "",
            ),
        ],
    )
