"""Analysis tests that unsupported Qt targets fail with their reason."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//qt/sdk:pins.bzl", "UNSUPPORTED_TARGETS")

def _unsupported_test_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, "Qt target '" + ctx.attr.target_name + "' is unsupported: " + UNSUPPORTED_TARGETS[ctx.attr.target_name])
    return analysistest.end(env)

_unsupported_test = analysistest.make(
    _unsupported_test_impl,
    expect_failure = True,
    attrs = {
        "target_name": attr.string(mandatory = True),
    },
)

def qt_unsupported_tests(name):
    """Declares one failing-analysis test per unsupported Qt target."""
    tests = []
    for target in sorted(UNSUPPORTED_TARGETS.keys()):
        test = name + "_" + target
        _unsupported_test(
            name = test,
            target_name = target,
            target_under_test = "@dx_qt//:" + target,
            target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
        )
        tests.append(":" + test)
    native.test_suite(name = name, tests = tests)
