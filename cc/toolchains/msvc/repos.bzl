"""MSVC toolchains over the windows_support repositories behind a license acceptance."""

load("//cc/toolchains:pins.bzl", "MSVC_TOOLSET_VERSION", "VC_REDIST_VERSION", "WINDOWS_SDK_INCLUDE_VERSION")
load("//libs/starlark:license.bzl", "accepted", "license_tag", "unaccepted")
load("//libs/starlark:repo.bzl", "hub_repo")
load(":layout.bzl", "SUPPORTED_CELLS")

HUB_NAME = "dx_msvc"
LICENSE = "visual-studio-build-tools"
LICENSE_URL = "https://visualstudio.microsoft.com/license-terms/"

UNACCEPTED = unaccepted("dx_msvc", "Visual Studio Build Tools license", LICENSE_URL, "msvc", LICENSE)

def cell_name(host, target):
    """Returns the hub target name for a host and target architecture."""
    return "cc_toolchain_{}_{}".format(host, target)

def cell_files(vc_repo, sdk_repo, host, target):
    """Returns the labels of the msvc_runtime and windows_sdk files one cell needs.

    Args:
        vc_repo: Canonical name of the msvc_runtime repository.
        sdk_repo: Canonical name of the windows_sdk repository.
        host: Architecture the tools run on.
        target: Architecture the tools build for.

    Returns:
        A list of label strings.
    """
    vc = "@@{}//:".format(vc_repo)
    sdk = "@@{}//:".format(sdk_repo)
    return [
        vc + "msvc_include_files",
        vc + "msvc_lib_{}_files".format(target),
        vc + "msvc_host_tools",
    ] + [
        sdk + "winsdk_{}_include_files".format(d)
        for d in ["ucrt", "shared", "um", "winrt"]
    ] + [
        sdk + "winsdk_{}_lib_{}_files".format(d, target)
        for d in ["ucrt", "um"]
    ] + [sdk + "winsdk_bin_" + host]

def hub_build(accepted, vc_repo = "", sdk_repo = "", msvc_version = MSVC_TOOLSET_VERSION, sdk_version = WINDOWS_SDK_INCLUDE_VERSION, redist_version = VC_REDIST_VERSION):
    """Returns the BUILD text of the hub repository.

    Args:
        accepted: Whether the license is accepted.
        vc_repo: Canonical name of the msvc_runtime repository.
        sdk_repo: Canonical name of the windows_sdk repository.
        msvc_version: MSVC toolset version directory.
        sdk_version: Windows SDK Include directory name.
        redist_version: Visual C++ redistributable version directory.

    Returns:
        The BUILD file text with one target per supported (host, target) cell.
    """
    text = 'load("{}", "dx_native_runtime")\n\npackage(default_visibility = ["//visibility:public"])\n\n'.format(str(Label("//deploy/rules:native_runtime.bzl")))
    if accepted:
        text = 'load("{}", "msvc_cc_toolchain")\n'.format(str(Label(":toolchain.bzl"))) + text
    else:
        text = 'load("{}", "cc_toolchain_unavailable")\n\n'.format(str(Label("//cc/toolchains:unavailable.bzl"))) + text
    for host, target in SUPPORTED_CELLS:
        name = cell_name(host, target)
        if accepted:
            text += 'msvc_cc_toolchain(\n    name = "{}",\n    msvc_version = "{}",\n    sdk_version = "{}",\n    vc_repo = "{}",\n    sdk_repo = "{}",\n    files = {},\n    exec_cpu = "{}",\n    target_cpu = "{}",\n)\n\n'.format(
                name,
                msvc_version,
                sdk_version,
                vc_repo,
                sdk_repo,
                json.encode(cell_files(vc_repo, sdk_repo, host, target)),
                host,
                target,
            )
        else:
            text += 'cc_toolchain_unavailable(\n    name = "{}",\n    error = {},\n)\n\n'.format(name, json.encode(UNACCEPTED))
    for arch in ["x64", "arm64"]:
        files = ["@@{}//:vc_runtime_dlls_{}".format(vc_repo, arch)] if accepted else []
        text += 'filegroup(\n    name = "vcruntime_{}",\n    srcs = {},\n)\n\n'.format(arch, json.encode(files))
    text += 'config_setting(\n    name = "windows_arm64",\n    constraint_values = [\n        "@platforms//os:windows",\n        "@platforms//cpu:arm64",\n    ],\n)\n\n'
    if accepted:
        text += 'dx_native_runtime(\n    name = "msvc_vcruntime",\n    srcs = select({{\n        ":windows_arm64": [":vcruntime_arm64"],\n        "//conditions:default": [":vcruntime_x64"],\n    }}),\n    notice = "{}",\n    redistribution = "app_local",\n    source = "Visual C++ Redistributable",\n    version = "{}",\n)\n\n'.format(str(Label("//cc/toolchains:msvc_vcruntime_notice")), redist_version)
    else:
        text += 'cc_toolchain_unavailable(\n    name = "msvc_vcruntime",\n    error = {},\n)\n\n'.format(json.encode(UNACCEPTED))
    return text.rstrip("\n") + "\n"

def _extension_impl(mctx):
    if accepted(mctx, "dx_msvc", LICENSE):
        files = {
            "BUILD.bazel": hub_build(True, Label("@msvc_runtime//:BUILD.bazel").repo_name, Label("@windows_sdk//:BUILD.bazel").repo_name),
            "NOTICE": "Microsoft Visual C++ and Windows SDK files, licensed under {}\nMSVC {}\nWindows SDK {}\n".format(LICENSE_URL, MSVC_TOOLSET_VERSION, WINDOWS_SDK_INCLUDE_VERSION),
            "REPO.bazel": "",
        }
    else:
        files = {"BUILD.bazel": hub_build(False)}
    hub_repo(name = HUB_NAME, files = files)
    return mctx.extension_metadata(reproducible = True)

_license = license_tag("Accepts the Visual Studio Build Tools license for this build.")

msvc = module_extension(
    implementation = _extension_impl,
    doc = "Declares the MSVC toolchain hub repository.",
    tag_classes = {"license": _license},
)
