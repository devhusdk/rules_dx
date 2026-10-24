"""Hermetic AppImage tool acquisition."""

load(":pins.bzl", "ARCHES", "RUNTIMES", "TOOLS", "TOOL_BINARIES")

HUB_NAME = "dx_appimage_tools"

_BUILD = """package(default_visibility = ["//visibility:public"])

filegroup(
    name = "tools",
    srcs = [%s],
)

filegroup(
    name = "runtime_x86_64",
    srcs = ["runtime-x86_64"],
)

filegroup(
    name = "runtime_aarch64",
    srcs = ["runtime-aarch64"],
)
"""

_UNSUPPORTED_BUILD = """package(default_visibility = ["//visibility:public"])

[filegroup(
    name = name,
    target_compatible_with = ["@platforms//:incompatible"],
) for name in ["tools", "runtime_x86_64", "runtime_aarch64"]]
"""

def unsupported_build():
    """Returns the BUILD text of the tool repository on a host without pinned tools."""
    return _UNSUPPORTED_BUILD

def hub_build():
    """Returns the BUILD text of the tool repository."""
    names = ", ".join(['"bin/' + name + '"' for name in TOOL_BINARIES])
    return _BUILD % names

def host_arch(os_name, arch):
    """Returns the pinned arch name of a Linux host, or an empty string."""
    if not os_name.startswith("linux"):
        return ""
    return {"aarch64": "aarch64", "amd64": "x86_64", "arm64": "aarch64", "x86_64": "x86_64"}.get(arch, "")

def host_error(os_name, arch):
    """Returns why a host cannot acquire the tools, or an empty string."""
    if not host_arch(os_name, arch):
        return "AppImage tools are only pinned for linux_x86_64 and linux_arm64, not " + os_name + "_" + arch
    return ""

def _tools_impl(rctx):
    if host_error(rctx.os.name, rctx.os.arch):
        rctx.file("BUILD.bazel", unsupported_build())
        return
    tool = TOOLS[host_arch(rctx.os.name, rctx.os.arch)]
    rctx.download(
        url = [tool["url"]],
        output = "appimagetool.AppImage",
        sha256 = tool["sha256"],
        executable = True,
    )
    for arch in ARCHES:
        rctx.download(
            url = [RUNTIMES[arch]["url"]],
            output = "runtime-" + arch,
            sha256 = RUNTIMES[arch]["sha256"],
            executable = True,
        )
    rctx.file("extract/.keep", "")
    result = rctx.execute(
        [rctx.path("appimagetool.AppImage"), "--appimage-extract"],
        working_directory = "extract",
    )
    if result.return_code != 0:
        fail("rules_dx: extracting the appimagetool image failed: " + result.stderr)
    for name in TOOL_BINARIES:
        rctx.symlink("extract/squashfs-root/usr/bin/" + name, "bin/" + name)
    rctx.delete("appimagetool.AppImage")
    rctx.file("BUILD.bazel", hub_build())

_tools = repository_rule(
    implementation = _tools_impl,
    doc = "Downloads the pinned appimagetool and the type 2 runtimes.",
)

def _extension_impl(mctx):
    _tools(name = HUB_NAME)
    return mctx.extension_metadata(reproducible = True)

appimage_tools = module_extension(
    implementation = _extension_impl,
    doc = "Declares the pinned AppImage tool repository.",
)
