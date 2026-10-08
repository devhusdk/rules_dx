"""Unit and analysis tests for the PyPI publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":pypi.bzl", "PYPI_DEFAULT_REPOSITORY_URL", "pypi_name_error", "pypi_repository_error", "pypi_schema_error", "pypi_wheel_error")

def pypi_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
            expect_equal(
                "pypi_schema_error accepts the v1 launcher-safe charset",
                pypi_schema_error(),
                "",
            ),
            expect_equal(
                "pypi_name_error accepts the demo distribution name",
                pypi_name_error("pypi_demo"),
                "",
            ),
            expect_equal(
                "pypi_name_error accepts dotted names",
                pypi_name_error("dx.plugin"),
                "",
            ),
            expect_equal(
                "pypi_name_error rejects an empty name",
                pypi_name_error(""),
                "pypi_deploy: invalid distribution name '': want a non-empty " +
                "name (for example 'pypi_demo')",
            ),
            expect_equal(
                "pypi_name_error rejects launcher-unsafe name characters",
                pypi_name_error("dx;curl evil"),
                "pypi_deploy: invalid distribution name 'dx;curl evil': want " +
                "only [A-Za-z0-9._-] so the name embeds safely in the deploy launcher",
            ),
            expect_equal(
                "pypi_repository_error accepts the default index URL",
                pypi_repository_error(PYPI_DEFAULT_REPOSITORY_URL),
                "",
            ),
            expect_equal(
                "pypi_repository_error rejects plaintext URLs",
                pypi_repository_error("http://example.com/legacy/"),
                "pypi_deploy: invalid repository_url 'http://example.com/legacy/': " +
                "want an https URL so uploads never go over plaintext",
            ),
            expect_equal(
                "pypi_repository_error rejects an empty URL",
                pypi_repository_error(""),
                "pypi_deploy: invalid repository_url '': want a non-empty https " +
                "URL (for example '" + PYPI_DEFAULT_REPOSITORY_URL + "')",
            ),
            expect_equal(
                "pypi_wheel_error accepts a pinned wheel filename",
                pypi_wheel_error("pypi_demo-0.0.0-py3-none-any.whl"),
                "",
            ),
            expect_equal(
                "pypi_wheel_error rejects a non-wheel filename",
                pypi_wheel_error("pypi_demo-0.0.0.tar.gz"),
                "pypi_deploy: invalid wheel 'pypi_demo-0.0.0.tar.gz': want a " +
                "filename ending in .whl",
            ),
        ],
    )

EXPECTED_PYPI_DEFAULT_OBSERVATIONS = """subject //deploy/rules:pypi_demo
file pypi_demo
field app=
field artifacts=
field profile=release
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:pypi_demo
aspect_field transitive_count=0"""

EXPECTED_PYPI_DEBUG_OBSERVATIONS = """subject //deploy/rules:pypi_demo_debug
file pypi_demo_debug
field app=
field artifacts=
field profile=debug
aspect_field aspect_seen=True
aspect_field field_count=3
aspect_field has_subject=True
aspect_field subject_label=//deploy/rules:pypi_demo_debug
aspect_field transitive_count=0"""

def pypi_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":pypi_demo"],
        expected_observations = EXPECTED_PYPI_DEFAULT_OBSERVATIONS,
    )

def pypi_debug_analysis_tests(name):
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":pypi_demo_debug"],
        expected_observations = EXPECTED_PYPI_DEBUG_OBSERVATIONS,
    )
