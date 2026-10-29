"""Analysis tests for capnp_library."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load(":defs.bzl", "CapnpInfo", "capnp_library", "std_import_path")

def _closure_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[CapnpInfo]
    paths = sorted([i.path for i in info.transitive_imports.to_list()])
    asserts.equals(env, ["capnproto/rules/testdata/base.capnp", "capnproto/rules/testdata/top.capnp"], [p for p in paths if not p.startswith("capnp/")])
    asserts.equals(env, ["capnp/c++.capnp", "capnp/compat/json.capnp", "capnp/java.capnp", "capnp/schema.capnp", "capnp/stream.capnp"], [p for p in paths if p.startswith("capnp/")])
    asserts.equals(env, ["top.capnp"], [f.basename for f in info.direct_srcs])
    asserts.equals(env, ["base.capnp", "top.capnp"], sorted([f.basename for f in info.transitive_srcs.to_list()]))
    asserts.equals(env, "capnp/compat/json.capnp", std_import_path("../+http_archive+dx_capnproto/src/capnp/compat/json.capnp"))
    asserts.equals(env, "capnp/java.capnp", std_import_path("../+http_archive+dx_capnproto_java/compiler/src/main/schema/capnp/java.capnp"))
    return analysistest.end(env)

_closure_test = analysistest.make(_closure_impl)

def _external_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, "cross-repo capnp schemas are unsupported")
    return analysistest.end(env)

_external_test = analysistest.make(_external_impl, expect_failure = True)

def capnp_library_tests(name):
    """Instantiates the capnp_library analysis tests.

    Args:
      name: Name of the test suite.
    """
    capnp_library(name = name + "_base", srcs = ["testdata/base.capnp"], tags = ["manual"])
    capnp_library(name = name + "_top", srcs = ["testdata/top.capnp"], deps = [":" + name + "_base"], tags = ["manual"])
    _closure_test(name = name + "_closure_test", target_under_test = ":" + name + "_top")
    capnp_library(name = name + "_external", srcs = ["//capnproto:std"], tags = ["manual"])
    _external_test(name = name + "_external_test", target_under_test = ":" + name + "_external")
    native.test_suite(name = name, tests = [":" + name + "_closure_test", ":" + name + "_external_test"])
