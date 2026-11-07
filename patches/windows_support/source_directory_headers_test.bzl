"""Tests for source_directory_headers.patch and the dx_msvc_layout.patch subsets."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("@bazel_skylib//rules/directory:providers.bzl", "DirectoryInfo")

def _source_directory_headers_test_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    files = target[DefaultInfo].files.to_list()
    asserts.equals(env, 1, len(files))
    if len(files) == 1:
        asserts.true(env, files[0].is_source, files[0].path + " is not a source file")
        asserts.true(env, files[0].path.endswith(ctx.attr.suffix), files[0].path)
        asserts.equals(env, files[0].path, target[DirectoryInfo].path)
    return analysistest.end(env)

_source_directory_headers_test = analysistest.make(
    _source_directory_headers_test_impl,
    attrs = {"suffix": attr.string(mandatory = True)},
)

_WINDOWS_SDK = {
    "winsdk_ucrt_lib_arm64": "/arm64/c/ucrt/arm64",
    "winsdk_ucrt_lib_x64": "/x64/c/ucrt/x64",
    "winsdk_um_include": "/um",
    "winsdk_um_lib_arm64": "/arm64/c/um/arm64",
    "winsdk_um_lib_x64": "/x64/c/um/x64",
}

_MSVC_RUNTIME = {
    "msvc_com_support_include": "/dx_subsets/com_support_include",
    "msvc_vcruntime_include": "/dx_subsets/vcruntime_include",
} | {
    "msvc_{}_libraries_{}".format(subset, arch): "/dx_subsets/lib/{}/{}".format(arch, subset)
    for subset in ("com", "common", "dynamic", "static")
    for arch in ("x64", "arm64")
}

def source_directory_headers_test(name):
    """Checks that Windows SDK and MSVC subset targets reach actions as one source directory each.

    Args:
        name: Name of the test suite.
    """
    tests = []
    for target, suffix in _WINDOWS_SDK.items():
        tests.append(name + "_" + target)
        _source_directory_headers_test(
            name = tests[-1],
            target_under_test = "@windows_sdk//:" + target,
            suffix = suffix,
        )

    # @msvc_runtime accepts the MSVC license only on Windows.
    for target, suffix in _MSVC_RUNTIME.items():
        tests.append(name + "_" + target)
        _source_directory_headers_test(
            name = tests[-1],
            target_under_test = "@msvc_runtime//:" + target,
            suffix = suffix,
            target_compatible_with = ["@platforms//os:windows"],
        )
    native.test_suite(
        name = name,
        tests = tests,
    )
