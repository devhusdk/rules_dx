"""The MSVC part of the cc extension."""

load("//cc/private/toolchain:pins.bzl", "MSVC_TOOLSET_VERSION", "WINDOWS_SDK_INCLUDE_VERSION")
load("//core:license.bzl", "accepted")
load("//core:repo.bzl", "hub_repo")
load(":repos.bzl", "HUB_NAME", "LICENSE", "LICENSE_URL", "hub_build")


def msvc_hub(mctx):
    """Declares the MSVC toolchain hub repository."""
    if accepted(mctx, LICENSE):
        files = {
            "BUILD.bazel": hub_build(True, Label("@dx_msvc_runtime//:BUILD.bazel").repo_name, Label("@dx_windows_sdk//:BUILD.bazel").repo_name),
            "NOTICE": "Microsoft Visual C++ and Windows SDK files, licensed under {}\nMSVC {}\nWindows SDK {}\n".format(LICENSE_URL, MSVC_TOOLSET_VERSION, WINDOWS_SDK_INCLUDE_VERSION),
            "REPO.bazel": "",
        }
    else:
        files = {"BUILD.bazel": hub_build(False)}
    hub_repo(name = HUB_NAME, files = files)
