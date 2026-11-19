"""Analysis tests for thrift_library."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@bazel_skylib//rules:build_test.bzl", "build_test")
load("//core:failure_test.bzl", "failure_test")
load("//thrift/private:codegen.bzl", "thrift_cc_library")
load("//thrift/private:defs.bzl", "ThriftInfo", "thrift_library")

def _closure_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[ThriftInfo]
    paths = sorted([i.path for i in info.transitive_includes.to_list()])
    asserts.equals(env, ["thrift/tests/fixtures/closure/base.thrift", "thrift/tests/fixtures/closure/top.thrift"], paths)
    asserts.equals(env, ["top.thrift"], [f.basename for f in info.direct_srcs])
    asserts.equals(env, ["base.thrift", "top.thrift"], sorted([f.basename for f in info.transitive_srcs.to_list()]))
    asserts.equals(env, 1, len(info.direct_deps))
    asserts.equals(env, "_base", info.direct_deps[0].name[-5:])
    return analysistest.end(env)

_closure_test = analysistest.make(_closure_impl)

def _chain_impl(ctx):
    env = analysistest.begin(ctx)
    info = analysistest.target_under_test(env)[ThriftInfo]
    symlinks = [a for a in analysistest.target_actions(env) if a.mnemonic == "Symlink"]
    asserts.equals(env, ["leaf.thrift"], [f.basename for a in symlinks for f in a.outputs.to_list()])
    asserts.equals(env, ["base.thrift", "leaf.thrift", "top.thrift"], sorted([f.basename for f in info.staged.to_list()]))
    asserts.equals(env, 1, len(info.stage_roots.to_list()))
    return analysistest.end(env)

_chain_test = analysistest.make(_chain_impl)

def thrift_library_tests(name):
    """Instantiates the thrift_library analysis tests."""
    thrift_library(name = name + "_base", srcs = ["fixtures/closure/base.thrift"], tags = ["manual"])
    thrift_library(name = name + "_top", srcs = ["fixtures/closure/top.thrift"], deps = [":" + name + "_base"], tags = ["manual"])
    _closure_test(name = name + "_closure_test", target_under_test = ":" + name + "_top", size = "small")
    thrift_library(name = name + "_leaf", srcs = ["fixtures/closure/leaf.thrift"], deps = [":" + name + "_top"], tags = ["manual"])
    _chain_test(name = name + "_chain_test", target_under_test = ":" + name + "_leaf", size = "small")
    thrift_cc_library(name = name + "_leaf_cc", deps = [":" + name + "_leaf"])
    build_test(name = name + "_leaf_cc_test", targets = [":" + name + "_leaf_cc"])
    thrift_library(name = name + "_external", srcs = ["@dx_thrift//:sample_schema"], tags = ["manual"])
    failure_test(name = name + "_external_test", size = "small", target = ":" + name + "_external", expected_failure_substring = "cross-repo thrift schemas are unsupported")
    native.test_suite(name = name, tests = [":" + name + "_closure_test", ":" + name + "_chain_test", ":" + name + "_leaf_cc_test", ":" + name + "_external_test"])
