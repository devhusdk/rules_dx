"""Analysis tests for exec_os."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load(":defs.bzl", "ExecOsInfo")

def _windows_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.true(env, analysistest.target_under_test(env)[ExecOsInfo].windows)
    return analysistest.end(env)

def _default_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.false(env, analysistest.target_under_test(env)[ExecOsInfo].windows)
    return analysistest.end(env)

_windows_test = analysistest.make(
    _windows_impl,
    config_settings = {"//command_line_option:platforms": [str(Label(":windows_x64"))]},
)
_default_test = analysistest.make(
    _default_impl,
    config_settings = {"//command_line_option:platforms": [str(Label(":linux_x64"))]},
)

def exec_os_tests(name):
    """Declares the exec_os analysis tests.

    Args:
        name: Name of the test suite.
    """
    tests = [name + "_windows", name + "_default"]
    _windows_test(name = tests[0], size = "small", target_under_test = ":exec_os")
    _default_test(name = tests[1], size = "small", target_under_test = ":exec_os")
    native.test_suite(name = name, tests = tests)
