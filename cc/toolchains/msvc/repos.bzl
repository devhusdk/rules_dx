"""MSVC layout assembled from the windows_support repositories behind a license acceptance."""

load("//libs/starlark:license.bzl", "accepted", "license_tag", "unaccepted")
load("//libs/starlark:repo.bzl", "hub_repo")
load(":layout.bzl", "SUPPORTED_CELLS", "msvc_layout")

HUB_NAME = "dx_msvc"
LICENSE = "visual-studio-build-tools"
LICENSE_URL = "https://visualstudio.microsoft.com/license-terms/"
VCRUNTIME_DLLS = ["vcruntime140", "vcruntime140_1", "msvcp140", "msvcp140_1", "msvcp140_2"]
VCVARS_STUBS = ["VC/Auxiliary/Build/vcvarsall.bat", "VC/Auxiliary/Build/vcvars64.bat"]

NO_REDIST = "dx_msvc: msvc_runtime has no VC/Redist/MSVC folder; the Visual C++ runtime cannot ship"

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
    vc = "VC/Tools/MSVC/" + msvc_version
    vc_src = "sysroot/Contents/" + vc
    hosts = sorted({host: None for host, _ in SUPPORTED_CELLS})
    return [
        struct(dst = vc + "/include", repo = "msvc_runtime", src = vc_src + "/include"),
        struct(dst = vc + "/lib", repo = "msvc_runtime", src = vc_src + "/lib"),
    ] + [
        struct(dst = vc + "/bin/Host" + host, repo = "msvc_runtime", src = vc_src + "/bin/Host" + host)
        for host in hosts
    ] + [
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

def match_name(parent, name):
    """Returns the entry of parent whose name equals name ignoring case, or name when none does."""
    if parent.exists:
        for child in parent.readdir():
            if child.basename.lower() == name.lower():
                return child.basename
    return name

def missing_error(paths):
    """Returns the error for tools missing from the assembled layout."""
    return "dx_msvc: incomplete MSVC payload; missing " + ", ".join(paths)

def has_child_exact(names, name):
    """Returns whether name is in names, matching letter case exactly."""
    return name in names

def cell_name(host, target):
    """Returns the hub target name for a host and target architecture."""
    return "cc_toolchain_{}_{}".format(host, target)

def hub_build(accepted, msvc_version, sdk_version, unavailable = {}, redist_version = ""):
    """Returns the BUILD text of the layout repository.

    Args:
        accepted: Whether the license is accepted.
        msvc_version: MSVC toolset version directory.
        sdk_version: Windows SDK version directory.
        unavailable: Map of cell target name to the error for cells whose tools are missing.
        redist_version: Visual C++ redistributable version directory, or "" when none ships.

    Returns:
        The BUILD file text with one target per supported (host, target) cell.
    """
    text = 'load("{}", "native_runtime")\n\npackage(default_visibility = ["//visibility:public"])\n\n'.format(str(Label("//deploy/rules:native_runtime.bzl")))
    if accepted:
        text = 'load("{}", "msvc_cc_toolchain")\nload("{}", "cc_toolchain_unavailable")\n\n'.format(str(Label(":toolchain.bzl")), str(Label("//cc/toolchains:unavailable.bzl"))) + text
    else:
        text = 'load("{}", "cc_toolchain_unavailable")\n\n'.format(str(Label("//cc/toolchains:unavailable.bzl"))) + text
    for host, target in SUPPORTED_CELLS:
        name = cell_name(host, target)
        error = unavailable.get(name) or (None if accepted else UNACCEPTED)
        if error:
            text += 'cc_toolchain_unavailable(\n    name = "{}",\n    error = {},\n)\n\n'.format(name, json.encode(error))
        else:
            text += 'msvc_cc_toolchain(\n    name = "{}",\n    msvc_version = "{}",\n    sdk_version = "{}",\n    exec_cpu = "{}",\n    target_cpu = "{}",\n)\n\n'.format(name, msvc_version, sdk_version, host, target)
    for arch in ["x64", "arm64"]:
        files = ["redist/{}/*/{}.dll".format(arch, dll) for dll in VCRUNTIME_DLLS] if accepted else []
        text += 'filegroup(\n    name = "vcruntime_{}",\n    srcs = glob({}),\n)\n\n'.format(arch, json.encode(files))
    text += 'config_setting(\n    name = "windows_arm64",\n    constraint_values = [\n        "@platforms//os:windows",\n        "@platforms//cpu:arm64",\n    ],\n)\n\n'
    if accepted and redist_version:
        text += 'native_runtime(\n    name = "msvc_vcruntime",\n    srcs = select({{\n        ":windows_arm64": [":vcruntime_arm64"],\n        "//conditions:default": [":vcruntime_x64"],\n    }}),\n    notice = "{}",\n    redistribution = "app_local",\n    source = "Visual C++ Redistributable",\n    version = "{}",\n)\n\n'.format(str(Label("//cc/toolchains:msvc_vcruntime_notice")), redist_version)
    else:
        text += 'cc_toolchain_unavailable(\n    name = "msvc_vcruntime",\n    error = {},\n)\n\n'.format(json.encode(NO_REDIST if accepted else UNACCEPTED))
    return text.rstrip("\n") + "\n"

def _only_child(rctx, root):
    names = [p.basename for p in root.readdir()]
    if len(names) != 1:
        fail("dx_msvc: expected one version under {}, found {}".format(root, ", ".join(sorted([str(n) for n in names])) or "none"))
    return names[0]

def _exists_exact(rctx, rel):
    cur = rctx.path("")
    for part in rel.split("/"):
        if not cur.exists or not has_child_exact([c.basename for c in cur.readdir()], part):
            return False
        cur = cur.get_child(part)
    return True

def host_dir_name(name):
    """Returns the canonical spelling of a bin host directory; payloads spell Hostx64 and HostX64 both."""
    if name.lower().startswith("host"):
        return "Host" + name[4:].lower()
    return name

def _link_toolset(rctx, source, dst):
    """Links the toolset entry by entry so bin gets canonical host directory names."""
    for entry in source.readdir():
        if entry.basename == "bin":
            for host in entry.readdir():
                target = "{}/bin/{}".format(dst, host_dir_name(host.basename))
                if not rctx.path(target).exists:
                    rctx.symlink(host, target)
        else:
            rctx.symlink(entry, "{}/{}".format(dst, entry.basename))

def _present(rctx, path):
    parent = path
    for _ in range(path.count("/") + 1):
        parent = parent.rpartition("/")[0]
        if rctx.path(parent).exists:
            return "; under {} found {}".format(parent or ".", ", ".join(sorted([str(c.basename) for c in rctx.path(parent).readdir()])) or "nothing")
    return ""

def _layout_repo_impl(rctx):
    vc_root = rctx.path(Label("@msvc_runtime//:BUILD.bazel")).dirname
    sdk_root = rctx.path(Label("@windows_sdk//:BUILD.bazel")).dirname
    roots = {"msvc_runtime": vc_root, "windows_sdk": sdk_root}
    msvc_version = _only_child(rctx, vc_root.get_child("sysroot/Contents/VC/Tools/MSVC"))
    sdk_version = _only_child(rctx, sdk_root.get_child("sysroot/base/c/Include"))
    for link in layout_links(msvc_version, sdk_version):
        source = roots[link.repo].get_child(link.src)
        source = source.dirname.get_child(match_name(source.dirname, source.basename))
        if source.exists and link.repo == "msvc_runtime":
            _link_toolset(rctx, source, link.dst)
        elif source.exists:
            rctx.symlink(source, link.dst)
    redist = rctx.path(Label("@msvc_runtime//:BUILD.bazel")).dirname.get_child("sysroot/Contents/VC/Redist/MSVC")
    redist_version = ""
    if redist.exists:
        redist_version = _only_child(rctx, redist)
        rctx.symlink(redist.get_child(redist_version), "redist")
    unavailable = {}
    for host, target in SUPPORTED_CELLS:
        layout = msvc_layout(msvc_version, sdk_version, host, target)
        missing = [p for p in [layout.cl, layout.link, layout.lib, layout.ml] if p and not _exists_exact(rctx, p)]
        if missing:
            unavailable[cell_name(host, target)] = missing_error(missing) + _present(rctx, missing[0])
    for stub in VCVARS_STUBS:
        rctx.file(stub, "@rem dx_msvc placeholder: nvcc probes for this file; -use-local-env keeps the hermetic environment.\n")
    rctx.file("NOTICE", "Microsoft Visual C++ and Windows SDK files, licensed under {}\nMSVC {}\nWindows SDK {}\n".format(LICENSE_URL, msvc_version, sdk_version))
    rctx.file("REPO.bazel", "")
    rctx.file("BUILD.bazel", hub_build(True, msvc_version, sdk_version, unavailable, redist_version))
    # The hub links by absolute path into other repos, so a cached copy has broken links.
    return rctx.repo_metadata(reproducible = False)

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
