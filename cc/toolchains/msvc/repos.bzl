"""MSVC layout assembled from the windows_support repositories behind a license acceptance."""

load("//libs/starlark:license.bzl", "accepted", "license_tag", "unaccepted")
load("//libs/starlark:repo.bzl", "hub_repo")
load(":layout.bzl", "msvc_layout")

HUB_NAME = "dx_msvc"
LICENSE = "visual-studio-build-tools"
LICENSE_URL = "https://visualstudio.microsoft.com/license-terms/"

UNACCEPTED = unaccepted("dx_msvc", "Visual Studio Build Tools license", LICENSE_URL, "msvc", LICENSE)

def layout_links(msvc_version, sdk_version):
    """Returns where each layout directory comes from.

    Args:
        msvc_version: MSVC toolset version directory.
        sdk_version: Windows SDK Include directory name, not the NuGet version.

    Returns:
        A list of structs with the layout path, the source repo and the path inside it.
    """
    sdk = "Windows Kits/10/"
    return [
        struct(dst = "VC/Tools/MSVC/" + msvc_version, repo = "msvc_runtime", src = "sysroot/Contents/VC/Tools/MSVC/" + msvc_version),
        struct(dst = sdk + "Include/" + sdk_version, repo = "windows_sdk", src = "sysroot/base/c/Include/" + sdk_version),
        struct(dst = sdk + "bin/" + sdk_version + "/x64", repo = "windows_sdk", src = "sysroot/base/c/bin/" + sdk_version + "/x64"),
        struct(dst = sdk + "Lib/" + sdk_version + "/ucrt/x64", repo = "windows_sdk", src = "sysroot/x64/c/ucrt/x64"),
        struct(dst = sdk + "Lib/" + sdk_version + "/um/x64", repo = "windows_sdk", src = "sysroot/x64/c/um/x64"),
    ]

def missing_error(paths):
    """Returns the error for tools missing from the assembled layout."""
    return "dx_msvc: incomplete MSVC payload; missing " + ", ".join(paths)

def hub_build(accepted, msvc_version, sdk_version):
    """Returns the BUILD text of the layout repository."""
    text = 'package(default_visibility = ["//visibility:public"])\n\n'
    if accepted:
        text = 'load("{}", "msvc_cc_toolchain")\n\n'.format(str(Label(":toolchain.bzl"))) + text
        return text + 'msvc_cc_toolchain(\n    name = "cc_toolchain",\n    msvc_version = "{}",\n    sdk_version = "{}",\n)\n'.format(msvc_version, sdk_version)
    text = 'load("{}", "msvc_unavailable")\n\n'.format(str(Label(":unavailable.bzl"))) + text
    return text + 'msvc_unavailable(\n    name = "cc_toolchain",\n    error = {},\n)\n'.format(json.encode(UNACCEPTED))

def _only_child(rctx, root):
    names = [p.basename for p in root.readdir()]
    if len(names) != 1:
        fail("dx_msvc: expected one version under {}, found {}".format(root, ", ".join(sorted([str(n) for n in names])) or "none"))
    return names[0]

def _layout_repo_impl(rctx):
    vc_root = rctx.path(Label("@msvc_runtime//:BUILD.bazel")).dirname
    sdk_root = rctx.path(Label("@windows_sdk//:BUILD.bazel")).dirname
    roots = {"msvc_runtime": vc_root, "windows_sdk": sdk_root}
    msvc_version = _only_child(rctx, vc_root.get_child("sysroot/Contents/VC/Tools/MSVC"))
    sdk_version = _only_child(rctx, sdk_root.get_child("sysroot/base/c/Include"))
    for link in layout_links(msvc_version, sdk_version):
        rctx.symlink(roots[link.repo].get_child(link.src), link.dst)
    layout = msvc_layout(msvc_version, sdk_version)
    missing = [p for p in [layout.cl, layout.link, layout.lib, layout.ml64] if not rctx.path(p).exists]
    if missing:
        fail(missing_error(missing))
    rctx.file("NOTICE", "Microsoft Visual C++ and Windows SDK files, licensed under {}\nMSVC {}\nWindows SDK {}\n".format(LICENSE_URL, msvc_version, sdk_version))
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", hub_build(True, msvc_version, sdk_version))
    return rctx.repo_metadata(reproducible = True)

_layout_repo = repository_rule(
    implementation = _layout_repo_impl,
    doc = "Links the MSVC toolset and Windows SDK into the layout msvc_cc_toolchain expects.",
)

def _extension_impl(mctx):
    if accepted(mctx, "dx_msvc", LICENSE):
        _layout_repo(name = HUB_NAME)
    else:
        hub_repo(name = HUB_NAME, files = {"BUILD.bazel": hub_build(False, "", "")})
    return mctx.extension_metadata(reproducible = True)

_license = license_tag("Accepts the Visual Studio Build Tools license for this build.")

msvc = module_extension(
    implementation = _extension_impl,
    doc = "Declares the MSVC layout repository.",
    tag_classes = {"license": _license},
)
