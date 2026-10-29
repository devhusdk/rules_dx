"""Analysis tests for thrift_library."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load(":defs.bzl", "ThriftInfo", "thrift_library")

def _closure_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[ThriftInfo]
    paths = sorted([i.path for i in info.transitive_includes.to_list()])
    asserts.equals(env, ["thrift/rules/testdata/base.thrift", "thrift/rules/testdata/top.thrift"], paths)
    asserts.equals(env, ["top.thrift"], [f.basename for f in info.direct_srcs])
    asserts.equals(env, ["base.thrift", "top.thrift"], sorted([f.basename for f in info.transitive_srcs.to_list()]))
    asserts.equals(env, 1, len(info.direct_deps))
    asserts.equals(env, "_base", info.direct_deps[0].name[-5:])
    return analysistest.end(env)

_closure_test = analysistest.make(_closure_impl)

def _external_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, "cross-repo thrift schemas are unsupported")
    return analysistest.end(env)

_external_test = analysistest.make(_external_impl, expect_failure = True)

def thrift_library_tests(name):
    """Instantiates the thrift_library analysis tests.

    Args:
      name: Name of the test suite.
    """
    thrift_library(name = name + "_base", srcs = ["testdata/base.thrift"], tags = ["manual"])
    thrift_library(name = name + "_top", srcs = ["testdata/top.thrift"], deps = [":" + name + "_base"], tags = ["manual"])
    _closure_test(name = name + "_closure_test", target_under_test = ":" + name + "_top")
    thrift_library(name = name + "_external", srcs = ["@dx_thrift//:sample_schema"], tags = ["manual"])
    _external_test(name = name + "_external_test", target_under_test = ":" + name + "_external")
    native.test_suite(name = name, tests = [":" + name + "_closure_test", ":" + name + "_external_test"])
