"""Analysis tests for protos outside the package of the generating rule."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//python/rules:defs.bzl", "python_grpc_library")
load("//typescript/rules:proto.bzl", "ts_proto_srcs")

def _other_package_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, "must be in the package")
    return analysistest.end(env)

_other_package_test = analysistest.make(_other_package_impl, expect_failure = True)

def proto_package_tests(name):
    """Instantiates the tests that reject a proto_library from another package.

    Args:
      name: Name of the test suite.
    """
    python_grpc_library(
        name = name + "_py",
        proto = "@rules_dx_js//typescript/tests/fixtures/proto:user_proto",
        py_proto = ":greeter_py_proto",
        tags = ["manual"],
        testonly = True,
    )
    ts_proto_srcs(
        name = name + "_ts_srcs",
        proto = "@rules_dx_js//typescript/tests/fixtures/proto:user_proto",
        tags = ["manual"],
        testonly = True,
    )
    _other_package_test(name = name + "_py_test", target_under_test = ":" + name + "_py_srcs")
    _other_package_test(name = name + "_ts_test", target_under_test = ":" + name + "_ts_srcs")
    native.test_suite(name = name, tests = [":" + name + "_py_test", ":" + name + "_ts_test"])
