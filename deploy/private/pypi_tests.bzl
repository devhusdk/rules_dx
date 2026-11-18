"""Unit and analysis tests for the PyPI publisher."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//core:defs.bzl", "expect_equal", "starlark_test")
load("//core/quality:unavailable_tool.bzl", "DxUnavailableToolInfo")
load(":validate_pypi.bzl", "PYPI_DEFAULT_REGISTRY_URL", "pypi_name_error", "pypi_registry_url_error", "pypi_wheel_error")

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
                "pypi_name_error rejects name characters",
                pypi_name_error("dx;curl evil"),
                "pypi_deploy: invalid distribution name 'dx;curl evil': want " +
                "only [A-Za-z0-9._-]",
            ),
            expect_equal(
                "pypi_registry_url_error accepts the default index URL",
                pypi_registry_url_error(PYPI_DEFAULT_REGISTRY_URL),
                "",
            ),
            expect_equal(
                "pypi_registry_url_error rejects plaintext URLs",
                pypi_registry_url_error("http://example.com/legacy/"),
                "pypi_deploy: invalid registry_url 'http://example.com/legacy/': " +
                "want an https URL so uploads never go over plaintext",
            ),
            expect_equal(
                "pypi_registry_url_error rejects an empty URL",
                pypi_registry_url_error(""),
                "pypi_deploy: invalid registry_url '': want a non-empty https " +
                "URL (for example '" + PYPI_DEFAULT_REGISTRY_URL + "')",
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

def _attestations_unavailable_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.true(env, DxUnavailableToolInfo in analysistest.target_under_test(env), "want DxUnavailableToolInfo on windows_arm64")
    return analysistest.end(env)

def _attestations_available_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.false(env, DxUnavailableToolInfo in analysistest.target_under_test(env), "want the pinned pypi-attestations on linux_x86_64")
    return analysistest.end(env)

_attestations_windows_arm64_test = analysistest.make(
    _attestations_unavailable_impl,
    config_settings = {"//command_line_option:platforms": str(Label("//tools/platforms:windows_arm64"))},
)

_attestations_linux_x86_64_test = analysistest.make(
    _attestations_available_impl,
    config_settings = {"//command_line_option:platforms": str(Label("//tools/platforms:linux_x86_64"))},
)

def pypi_attestations_tool_tests(name, size):
    """Checks the pypi-attestations alias picks the stub only on windows_arm64."""
    _attestations_windows_arm64_test(
        name = name + "_windows_arm64",
        size = size,
        target_under_test = "//core/quality/tools/python:pypi_attestations_tool",
    )
    _attestations_linux_x86_64_test(
        name = name + "_linux_x86_64",
        size = size,
        target_under_test = "//core/quality/tools/python:pypi_attestations_tool",
    )
    native.test_suite(
        name = name,
        tests = [name + "_windows_arm64", name + "_linux_x86_64"],
    )
