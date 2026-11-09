"""Analysis test that the starlark_test harness link is a plain symlink."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//libs/starlark:defs.bzl", "expect_equal", "starlark_test")

def _impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    executable = target[DefaultInfo].files_to_run.executable
    links = [a for a in analysistest.target_actions(env) if executable in a.outputs.to_list()]
    asserts.equals(env, 1, len(links))
    asserts.equals(env, "Symlink", links[0].mnemonic)
    return analysistest.end(env)

_harness_link_test = analysistest.make(_impl)

def harness_link_tests(name):
    """Instantiates the harness link analysis test.

    Args:
      name: Name of the test suite.
    """
    starlark_test(
        name = name + "_subject",
        mode = "unit",
        checks = [expect_equal("trivial", 1, 1)],
        tags = ["manual"],
    )
    _harness_link_test(name = name + "_test", target_under_test = ":" + name + "_subject")
    native.test_suite(name = name, tests = [":" + name + "_test"])
