"""Analysis tests for the Swift wrappers."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_swift//swift:providers.bzl", "SwiftInfo")

def _library_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    asserts.true(env, SwiftInfo in target)
    asserts.true(env, CcInfo in target)
    asserts.true(env, DefaultInfo in target)
    return analysistest.end(env)

def _binary_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    asserts.true(env, DefaultInfo in target)
    asserts.true(env, target[DefaultInfo].default_runfiles != None)
    asserts.true(env, target[DefaultInfo].files_to_run.executable != None)
    return analysistest.end(env)

swift_library_provider_test = analysistest.make(_library_impl)
swift_binary_provider_test = analysistest.make(_binary_impl)
