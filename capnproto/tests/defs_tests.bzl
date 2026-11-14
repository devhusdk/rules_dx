"""Analysis tests for capnproto_library."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//capnproto/private:defs.bzl", "capnproto_library", "std_import_path")
load("//capnproto/private:providers.bzl", "CapnprotoInfo")
load("//core:failure_test.bzl", "failure_test")

def _closure_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[CapnprotoInfo]
    paths = sorted([i.path for i in info.transitive_imports.to_list()])
    asserts.equals(env, ["capnproto/tests/fixtures/closure/base.capnp", "capnproto/tests/fixtures/closure/top.capnp"], [p for p in paths if not p.startswith("capnp/")])
    asserts.equals(env, ["capnp/c++.capnp", "capnp/compat/json.capnp", "capnp/java.capnp", "capnp/schema.capnp", "capnp/stream.capnp"], [p for p in paths if p.startswith("capnp/")])
    asserts.equals(env, ["top.capnp"], [f.basename for f in info.direct_srcs])
    asserts.equals(env, ["base.capnp", "top.capnp"], sorted([f.basename for f in info.transitive_srcs.to_list()]))
    asserts.equals(env, "capnp/compat/json.capnp", std_import_path("../+http_archive+dx_capnproto/src/capnp/compat/json.capnp"))
    asserts.equals(env, "capnp/java.capnp", std_import_path("../+http_archive+dx_capnproto_java/compiler/src/main/schema/capnp/java.capnp"))
    return analysistest.end(env)

_closure_test = analysistest.make(_closure_impl)

def capnp_library_tests(name):
    """Instantiates the capnproto_library analysis tests."""
    capnproto_library(name = name + "_base", srcs = ["fixtures/closure/base.capnp"], tags = ["manual"])
    capnproto_library(name = name + "_top", srcs = ["fixtures/closure/top.capnp"], deps = [":" + name + "_base"], tags = ["manual"])
    _closure_test(name = name + "_closure_test", target_under_test = ":" + name + "_top", size = "small")
    capnproto_library(name = name + "_external", srcs = ["//capnproto:std"], tags = ["manual"])
    failure_test(name = name + "_external_test", size = "small", target = ":" + name + "_external", expected_failure_substring = "cross-repo capnp schemas are unsupported")
    native.test_suite(name = name, tests = [":" + name + "_closure_test", ":" + name + "_external_test"])
