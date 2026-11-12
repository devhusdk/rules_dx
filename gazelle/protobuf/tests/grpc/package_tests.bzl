"""Analysis tests for protos outside the package of the generating rule."""

load("//core:failure_test.bzl", "failure_test")
load("//python:defs.bzl", "python_grpc_library")
load("//typescript/private:proto.bzl", "ts_proto_srcs")

def proto_package_tests(name):
    """Instantiates the tests that reject a proto_library from another package.

    Args:
      name: Name of the test suite.
    """
    python_grpc_library(
        name = name + "_py",
        proto = "//typescript/tests/fixtures/proto:user_proto",
        py_proto = ":greeter_py_proto",
        tags = ["manual"],
        testonly = True,
    )
    ts_proto_srcs(
        name = name + "_ts_srcs",
        proto = "//typescript/tests/fixtures/proto:user_proto",
        tags = ["manual"],
        testonly = True,
    )
    failure_test(name = name + "_py_test", target = ":" + name + "_py_srcs", expected_failure_substring = "must be in the package")
    failure_test(name = name + "_ts_test", target = ":" + name + "_ts_srcs", expected_failure_substring = "must be in the package")
    native.test_suite(name = name, tests = [":" + name + "_py_test", ":" + name + "_ts_test"])
