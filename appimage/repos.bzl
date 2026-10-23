"""Hermetic AppImage tool acquisition."""

load(":pins.bzl", "RUNTIME_SHA256", "RUNTIME_URL", "TOOL_BINARIES", "TOOL_SHA256", "TOOL_URL")

HUB_NAME = "dx_appimage_tools"

_BUILD = """package(default_visibility = ["//visibility:public"])

exports_files(["runtime"])

filegroup(
    name = "tools",
    srcs = [%s],
)

filegroup(
    name = "runtime_file",
    srcs = ["runtime"],
)
"""

def hub_build():
    """Returns the BUILD text of the tool repository."""
    names = ", ".join(['"bin/' + name + '"' for name in TOOL_BINARIES])
    return _BUILD % names

def host_error(os_name, arch):
    """Returns why a host cannot acquire the tools, or an empty string."""
    if not os_name.startswith("linux") or arch not in ["amd64", "x86_64"]:
        return "AppImage tools are only pinned for linux_x86_64, not " + os_name + "_" + arch
    return ""

def _tools_impl(rctx):
    error = host_error(rctx.os.name, rctx.os.arch)
    if error:
        fail("rules_dx: " + error)
    rctx.download(
        url = [TOOL_URL],
        output = "appimagetool.AppImage",
        sha256 = TOOL_SHA256,
        executable = True,
    )
    rctx.download(
        url = [RUNTIME_URL],
        output = "runtime",
        sha256 = RUNTIME_SHA256,
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
    doc = "Downloads the pinned appimagetool and type 2 runtime.",
)

def _extension_impl(mctx):
    _tools(name = HUB_NAME)
    return mctx.extension_metadata(reproducible = True)

appimage_tools = module_extension(
    implementation = _extension_impl,
    doc = "Declares the pinned AppImage tool repository.",
)
