"""Analysis tests for avro_library."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//avro/private:defs.bzl", "avro_library")
load("//avro/private:missing.bzl", "dx_missing_avro_cpp")
load("//avro/private:providers.bzl", "AvroInfo")

def _closure_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[AvroInfo]
    asserts.equals(env, ["top.avdl"], [f.basename for f in info.direct_srcs])
    asserts.equals(env, ["base.avsc", "top.avdl"], sorted([f.basename for f in info.transitive_srcs.to_list()]))
    asserts.equals(env, 1, len(info.direct_deps))
    return analysistest.end(env)

_closure_test = analysistest.make(_closure_impl)

def _extension_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, "is misplaced here")
    return analysistest.end(env)

_extension_test = analysistest.make(_extension_impl, expect_failure = True)

def _missing_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, "override_repo")
    return analysistest.end(env)

_missing_test = analysistest.make(_missing_impl, expect_failure = True)

def avro_library_tests(name):
    """Instantiates the avro_library analysis tests.

    Args:
      name: Name of the test suite.
    """
    avro_library(name = name + "_base", srcs = ["fixtures/closure/base.avsc"], tags = ["manual"])
    avro_library(name = name + "_top", srcs = ["fixtures/closure/top.avdl"], deps = [":" + name + "_base"], tags = ["manual"])
    _closure_test(name = name + "_closure_test", target_under_test = ":" + name + "_top", size = "small")
    avro_library(name = name + "_wrong", srcs = ["fixtures/closure/wrong.txt"], tags = ["manual"])
    _extension_test(name = name + "_extension_test", target_under_test = ":" + name + "_wrong", size = "small")
    dx_missing_avro_cpp(name = name + "_missing", tags = ["manual"])
    _missing_test(name = name + "_missing_test", target_under_test = ":" + name + "_missing", size = "small")
    native.test_suite(name = name, tests = [":" + name + "_closure_test", ":" + name + "_extension_test", ":" + name + "_missing_test"])
