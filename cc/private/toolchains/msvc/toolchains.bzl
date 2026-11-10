"""MSVC toolchain() targets, one per supported host and target pair."""

load(":layout.bzl", "SUPPORTED_CELLS")
load(":repos.bzl", "cell_name")

_CPU = {
    "arm64": "@platforms//cpu:arm64",
    "x64": "@platforms//cpu:x86_64",
}

_SUFFIX = {
    ("x64", "x64"): "x86_64",
    ("x64", "arm64"): "x86_64_to_arm64",
    ("arm64", "arm64"): "arm64",
}

def msvc_toolchain_cells(name = "msvc"):
    """Returns the (name, exec cpu, target cpu) of each supported cell.

    Args:
        name: Prefix of the toolchain names.

    Returns:
        A list of (toolchain name, exec cpu, target cpu) tuples.
    """
    return [
        ("{}_windows_{}".format(name, _SUFFIX[cell]), _CPU[cell[0]], _CPU[cell[1]])
        for cell in SUPPORTED_CELLS
    ]

def msvc_toolchains(name):
    """Declares a toolchain() for each supported MSVC cell.

    Args:
        name: Prefix of the toolchain names.
    """
    for cell in SUPPORTED_CELLS:
        host, target = cell
        native.toolchain(
            name = "{}_windows_{}".format(name, _SUFFIX[cell]),
            exec_compatible_with = ["@platforms//os:windows", _CPU[host]],
            target_compatible_with = ["@platforms//os:windows", _CPU[target]],
            target_settings = ["//cc/private/toolchains:msvc_selected"],
            toolchain = "@dx_msvc//:" + cell_name(host, target),
            toolchain_type = "@bazel_tools//tools/cpp:toolchain_type",
        )
