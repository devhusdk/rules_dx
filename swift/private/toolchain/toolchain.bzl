"""Registers the rules_swift Linux toolchain with the sysroot's link libraries."""

load("@rules_cc//cc:cc_library.bzl", "cc_library")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_swift//swift:providers.bzl", "SwiftToolchainInfo")

def _with_link_libs_impl(ctx):
    info = ctx.attr.toolchain[SwiftToolchainInfo]
    fields = {name: getattr(info, name) for name in dir(info) if name not in ("to_json", "to_proto")}
    extra = ctx.attr.link_libs[CcInfo]
    for name in ("dynamic_runtime_cc_info", "static_runtime_cc_info"):
        if fields[name]:
            fields[name] = cc_common.merge_cc_infos(cc_infos = [fields[name], extra])
    merged = SwiftToolchainInfo(**fields)
    return [
        platform_common.ToolchainInfo(swift_toolchain = merged),
        merged,
    ]

_with_link_libs = rule(
    implementation = _with_link_libs_impl,
    attrs = {
        "link_libs": attr.label(mandatory = True, providers = [CcInfo]),
        "toolchain": attr.label(mandatory = True, providers = [SwiftToolchainInfo]),
    },
)

_LINUX = ["@platforms//os:linux"]

def swift_linux_toolchain(name, cpu):
    """Declares the Swift toolchain and link libraries for one Linux cpu."""
    sysroot = "@dx_swift_sysroot_linux_" + cpu
    sysroot_root = Label(sysroot + "//:files").workspace_root
    tags = ["manual"]

    cc_library(
        name = "linux_link_libs_" + cpu,
        tags = tags,
        target_compatible_with = _LINUX,
        additional_linker_inputs = [
            sysroot + "//:link_libs/libcurl.so",
            sysroot + "//:link_libs/libgcc_eh.a",
            sysroot + "//:link_libs/libxml2.so",
        ],
        linkopts = [
            "-L" + sysroot_root + "/link_libs",
            "-lgcc_eh",
        ],
        srcs = [
            sysroot + "//:link_libs/libstdc++.a",
        ],
    )

    _with_link_libs(
        name = "swift_toolchain_" + cpu,
        tags = tags,
        target_compatible_with = _LINUX,
        link_libs = ":linux_link_libs_" + cpu,
        toolchain = "@dx_swift_linux_" + cpu + "//:swift_toolchain_exec",
    )

    native.toolchain(
        name = name,
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:" + cpu],
        target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:" + cpu],
        toolchain = ":swift_toolchain_" + cpu,
        toolchain_type = "@rules_swift//toolchains:toolchain_type",
    )
