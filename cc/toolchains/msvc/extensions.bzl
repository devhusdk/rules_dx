"""Module extension for cc/toolchains/msvc."""

load("//cc/toolchains:pins.bzl", "MSVC_TOOLSET_VERSION", "WINDOWS_SDK_INCLUDE_VERSION")
load("//core:license.bzl", "accepted", "license_tag")
load("//core:repo.bzl", "hub_repo")
load(":repos.bzl", "HUB_NAME", "LICENSE", "LICENSE_URL", "hub_build")


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
