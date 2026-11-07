"""MSVC hub repository coverage."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//cc/toolchains:unavailable.bzl", "cc_toolchain_unavailable")
load("//libs/starlark:defs.bzl", "expect_contains", "expect_equal", "expect_true", "starlark_test")
load(":repos.bzl", "HUB_NAME", "LICENSE", "UNACCEPTED", "cell_files", "hub_build")
load(":unavailable.bzl", "UNSUPPORTED")

_VC = "windows_support++msvc_runtime+msvc_runtime"
_SDK = "windows_support++windows_sdk+windows_sdk"

def _checks():
    accepted_hub = hub_build(True, _VC, _SDK, "14.44.35207", "10.0.26100.0", "14.50.35710")
    unaccepted_hub = hub_build(False)
    return [
        expect_equal("hub name", HUB_NAME, "dx_msvc"),
        expect_equal("cross cell files", cell_files(_VC, _SDK, "x64", "arm64"), [
            "@@" + _VC + "//:msvc_include_files",
            "@@" + _VC + "//:msvc_lib_arm64_files",
            "@@" + _VC + "//:msvc_host_tools",
            "@@" + _SDK + "//:winsdk_ucrt_include_files",
            "@@" + _SDK + "//:winsdk_shared_include_files",
            "@@" + _SDK + "//:winsdk_um_include_files",
            "@@" + _SDK + "//:winsdk_winrt_include_files",
            "@@" + _SDK + "//:winsdk_ucrt_lib_arm64_files",
            "@@" + _SDK + "//:winsdk_um_lib_arm64_files",
            "@@" + _SDK + "//:winsdk_bin_x64",
        ]),
        expect_equal(
            "unaccepted text",
            UNACCEPTED,
            "dx_msvc: the Visual Studio Build Tools license (https://visualstudio.microsoft.com/license-terms/) is not accepted; add msvc.license(accept = [\"visual-studio-build-tools\"]) to the root MODULE.bazel",
        ),
        expect_equal("license name", LICENSE, "visual-studio-build-tools"),
        expect_contains("x64 cell", accepted_hub, 'name = "cc_toolchain_x64_x64",\n    msvc_version = "14.44.35207",\n    sdk_version = "10.0.26100.0",\n    vc_repo = "' + _VC + '",\n    sdk_repo = "' + _SDK + '",\n    files = ' + json.encode(cell_files(_VC, _SDK, "x64", "x64")) + ',\n    exec_cpu = "x64",\n    target_cpu = "x64",'),
        expect_contains("native arm64 cell", accepted_hub, "files = " + json.encode(cell_files(_VC, _SDK, "arm64", "arm64")) + ',\n    exec_cpu = "arm64",\n    target_cpu = "arm64",'),
        expect_equal("one toolchain per cell", accepted_hub.count("msvc_cc_toolchain(\n"), 3),
        expect_true("accepted hub has no unavailable cell", "cc_toolchain_unavailable(" not in accepted_hub),
        expect_true("hub reads no repository at load time", "glob(" not in accepted_hub),
        expect_contains("x64 runtime from msvc_runtime", accepted_hub, 'name = "vcruntime_x64",\n    srcs = ["@@' + _VC + '//:vc_runtime_dlls_x64"],'),
        expect_contains("arm64 runtime from msvc_runtime", accepted_hub, 'name = "vcruntime_arm64",\n    srcs = ["@@' + _VC + '//:vc_runtime_dlls_arm64"],'),
        expect_contains("accepted hub names the runtime version", accepted_hub, 'version = "14.50.35710"'),
        expect_contains("accepted hub declares the runtime", accepted_hub, 'name = "msvc_vcruntime"'),
        expect_equal("unaccepted hub makes every cell and the runtime unavailable", unaccepted_hub.count("error = " + json.encode(UNACCEPTED)), 4),
        expect_contains("unaccepted hub ships no runtime", unaccepted_hub, 'name = "vcruntime_arm64",\n    srcs = [],'),
        expect_true("unaccepted hub builds no toolchain", "msvc_cc_toolchain(" not in unaccepted_hub),
        expect_true("unaccepted hub names no payload repository", "msvc_runtime" not in unaccepted_hub and "windows_sdk" not in unaccepted_hub),
        expect_true("unaccepted hub declares no runtime", "native_runtime(" not in unaccepted_hub),
    ]

def msvc_repos_tests(name):
    """Declares the MSVC layout assembly test."""
    starlark_test(
        name = name,
        mode = "load",
        checks = _checks(),
    )

def _unaccepted_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, UNACCEPTED)
    return analysistest.end(env)

_unaccepted_test = analysistest.make(_unaccepted_impl, expect_failure = True)

def msvc_unavailable_tests(name):
    """Declares the test that an unaccepted MSVC layout fails when used."""
    cc_toolchain_unavailable(
        name = name + "_target",
        error = UNACCEPTED,
        tags = ["manual"],
    )
    _unaccepted_test(
        name = name,
        target_under_test = ":" + name + "_target",
    )

def _unsupported_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, UNSUPPORTED)
    return analysistest.end(env)

_unsupported_test = analysistest.make(_unsupported_impl, expect_failure = True)

def msvc_unsupported_tests(name):
    """Declares the test that the unsupported-platform guard fails when used."""
    cc_toolchain_unavailable(
        name = name + "_target",
        error = UNSUPPORTED,
        tags = ["manual"],
    )
    _unsupported_test(
        name = name,
        target_under_test = ":" + name + "_target",
    )
