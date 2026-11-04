"""MSVC layout assembled from the windows_support repositories behind a license acceptance."""

load("//libs/starlark:license.bzl", "accepted", "license_tag", "unaccepted")
load("//libs/starlark:repo.bzl", "hub_repo")
load(":layout.bzl", "SUPPORTED_CELLS", "msvc_layout")

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
    ] + [
        link
        for arch in ["x64", "arm64"]
        for link in [
            struct(dst = sdk + "bin/" + sdk_version + "/" + arch, repo = "windows_sdk", src = "sysroot/base/c/bin/" + sdk_version + "/" + arch),
            struct(dst = sdk + "Lib/" + sdk_version + "/ucrt/" + arch, repo = "windows_sdk", src = "sysroot/{}/c/ucrt/{}".format(arch, arch)),
            struct(dst = sdk + "Lib/" + sdk_version + "/um/" + arch, repo = "windows_sdk", src = "sysroot/{}/c/um/{}".format(arch, arch)),
        ]
    ]

def missing_error(paths):
    """Returns the error for tools missing from the assembled layout."""
    return "dx_msvc: incomplete MSVC payload; missing " + ", ".join(paths)

def cell_name(host, target):
    """Returns the hub target name for a host and target architecture."""
    return "cc_toolchain_{}_{}".format(host, target)

def hub_build(accepted, msvc_version, sdk_version, unavailable = {}):
    """Returns the BUILD text of the layout repository.

    Args:
        accepted: Whether the license is accepted.
        msvc_version: MSVC toolset version directory.
        sdk_version: Windows SDK version directory.
        unavailable: Map of cell target name to the error for cells whose tools are missing.

    Returns:
        The BUILD file text with one target per supported (host, target) cell.
    """
    text = 'package(default_visibility = ["//visibility:public"])\n\n'
    if accepted:
        text = 'load("{}", "msvc_cc_toolchain")\nload("{}", "msvc_unavailable")\n\n'.format(str(Label(":toolchain.bzl")), str(Label(":unavailable.bzl"))) + text
    else:
        text = 'load("{}", "msvc_unavailable")\n\n'.format(str(Label(":unavailable.bzl"))) + text
    for host, target in SUPPORTED_CELLS:
        name = cell_name(host, target)
        error = unavailable.get(name) or (None if accepted else UNACCEPTED)
        if error:
            text += 'msvc_unavailable(\n    name = "{}",\n    error = {},\n)\n\n'.format(name, json.encode(error))
        else:
            text += 'msvc_cc_toolchain(\n    name = "{}",\n    msvc_version = "{}",\n    sdk_version = "{}",\n    exec_cpu = "{}",\n    target_cpu = "{}",\n)\n\n'.format(name, msvc_version, sdk_version, host, target)
    return text.rstrip("\n") + "\n"

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
        source = roots[link.repo].get_child(link.src)
        if source.exists:
            rctx.symlink(source, link.dst)
    unavailable = {}
    for host, target in SUPPORTED_CELLS:
        layout = msvc_layout(msvc_version, sdk_version, host, target)
        missing = [p for p in [layout.cl, layout.link, layout.lib, layout.ml] if p and not rctx.path(p).exists]
        if missing:
            unavailable[cell_name(host, target)] = missing_error(missing)
    rctx.file("NOTICE", "Microsoft Visual C++ and Windows SDK files, licensed under {}\nMSVC {}\nWindows SDK {}\n".format(LICENSE_URL, msvc_version, sdk_version))
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", hub_build(True, msvc_version, sdk_version, unavailable))
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
