"""Unit and analysis tests for the PyPI publisher."""

load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")
load(":pypi.bzl", "PYPI_DEFAULT_REPOSITORY_URL", "pypi_name_error", "pypi_repository_error", "pypi_wheel_error")

def pypi_unit_tests(name):
    starlark_test(
        name = name,
        mode = "unit",
        checks = [
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
