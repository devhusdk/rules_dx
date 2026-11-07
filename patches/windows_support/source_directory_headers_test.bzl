"""Tests for source_directory_headers.patch."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@bazel_skylib//rules/directory:providers.bzl", "DirectoryInfo")

def _source_directory_headers_test_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    files = target[DefaultInfo].files.to_list()
    asserts.equals(env, 1, len(files))
    if len(files) == 1:
        asserts.true(env, files[0].is_source, files[0].path + " is not a source file")
        asserts.true(env, files[0].path.endswith("/um"), files[0].path)
        asserts.equals(env, files[0].path, target[DirectoryInfo].path)
    return analysistest.end(env)

_source_directory_headers_test = analysistest.make(_source_directory_headers_test_impl)

def source_directory_headers_test(name):
    """Checks that the Windows SDK um headers reach compiles as one source directory."""
    _source_directory_headers_test(
        name = name,
        target_under_test = "@windows_sdk//:winsdk_um_include",
    )
