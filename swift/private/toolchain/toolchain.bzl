"""Registers the rules_swift Linux toolchains, native and cross, with the sysroot's link libraries."""

load("@rules_cc//cc:cc_library.bzl", "cc_library")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_swift//swift:providers.bzl", "SwiftToolchainInfo")
load("@rules_swift//swift/internal:action_names.bzl", "SWIFT_ACTION_DUMP_AST", "SWIFT_ACTION_PRECOMPILE_C_MODULE", "SWIFT_ACTION_SYMBOL_GRAPH_EXTRACT", "all_compile_action_names")
load("@rules_swift//swift/toolchains:swift_toolchain.bzl", "swift_toolchain")
load("@rules_swift//swift/toolchains:swift_tools.bzl", "swift_tools")
load("@rules_swift//swift/toolchains/config:action_config.bzl", "ActionConfigInfo", "add_arg")

def _with_link_libs_impl(ctx):
    info = ctx.attr.toolchain[SwiftToolchainInfo]
    fields = {name: getattr(info, name) for name in dir(info) if name not in ("to_json", "to_proto")}
    if ctx.attr.runtime:
        runtime = ctx.attr.runtime[SwiftToolchainInfo]
        fields["dynamic_runtime_cc_info"] = runtime.dynamic_runtime_cc_info
        fields["static_runtime_cc_info"] = runtime.static_runtime_cc_info
    if ctx.attr.resource_dir:
        fields["action_configs"] = info.action_configs + [ActionConfigInfo(
            actions = all_compile_action_names() + [
                SWIFT_ACTION_DUMP_AST,
                SWIFT_ACTION_PRECOMPILE_C_MODULE,
                SWIFT_ACTION_SYMBOL_GRAPH_EXTRACT,
            ],
            configurators = [add_arg("-Xwrapped-swift=-resource-dir=" + ctx.attr.resource_dir)],
        )]
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
        "resource_dir": attr.string(doc = "The Swift resource directory of the target cpu, when the tools run on another cpu."),
        "runtime": attr.label(
            doc = "The toolchain whose Swift runtime the target links, when the tools run on another cpu.",
            providers = [SwiftToolchainInfo],
        ),
        "toolchain": attr.label(mandatory = True, providers = [SwiftToolchainInfo]),
    },
)

_LINUX = ["@platforms//os:linux"]
_VERSION = "6.4.2"
SWIFT_LINUX_CPUS = ["aarch64", "x86_64"]

def _swift_cross_toolchain(name, exec_cpu, target_cpu):
    """Declares the Swift toolchain whose tools run on one Linux cpu and build for that or another."""
    exec_repo = "@dx_swift_linux_" + exec_cpu
    target_repo = "@dx_swift_linux_" + target_cpu
    exec_sysroot = "@dx_swift_sysroot_linux_" + exec_cpu
    target_sysroot = "@dx_swift_sysroot_linux_" + target_cpu

    swift_tools(
        name = name + "_tools",
        tags = ["manual"],
        additional_inputs = {
            label: True
            for label in [
                exec_repo + "//:swift_tool_inputs",
                exec_sysroot + "//:files",
                target_repo + "//:swift_tool_inputs",
                target_sysroot + "//:files",
            ]
        }.keys(),
        swift_autolink_extract = exec_repo + "//:usr/bin/swift-autolink-extract",
        swift_driver = exec_repo + "//:usr/bin/swiftc",
        swift_symbolgraph_extract = exec_repo + "//:usr/bin/swift-symbolgraph-extract",
    )

    swift_toolchain(
        name = name,
        tags = ["manual"],
        target_compatible_with = _LINUX,
        arch = target_cpu,
        dynamic_runtime = [target_repo + "//:linux_dynamic_runtime"],
        features = [
            "swift._supports_upcoming_features",
            "swift.no_embed_debug_module",
            "swift.use_autolink_extract",
            "swift.lld_gc_workaround",
            "swift.use_module_wrap",
            "swift.module_map_no_private_headers",
        ],
        os = "linux",
        parsed_version = _VERSION,
        sdkroot = Label(target_sysroot + "//:files").workspace_root,
        static_runtime = [target_repo + "//:linux_static_runtime"],
        swift_tools = name + "_tools",
        version_file = exec_repo + "//:.swift-version",
    )

def swift_linux_toolchain(name, exec_cpu, target_cpu):
    """Declares the Swift toolchain that runs on one Linux cpu and builds for another or the same.

    Args:
        name: The toolchain name.
        exec_cpu: The cpu the Swift tools run on.
        target_cpu: The cpu the Swift tools build for.
    """
    pair = exec_cpu + "_" + target_cpu
    sysroot = "@dx_swift_sysroot_linux_" + target_cpu
    sysroot_root = Label(sysroot + "//:files").workspace_root

    cc_library(
        name = "linux_link_libs_" + pair,
        tags = ["manual"],
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

    _swift_cross_toolchain("swift_cross_" + pair, exec_cpu, target_cpu)
    if exec_cpu == target_cpu:
        resource_dir = None
        runtime = None
    else:
        resource_dir = Label("@dx_swift_linux_" + target_cpu + "//:files").workspace_root + "/usr/lib/swift"
        runtime = ":swift_cross_" + target_cpu + "_" + target_cpu

    _with_link_libs(
        name = "swift_toolchain_" + pair,
        tags = ["manual"],
        target_compatible_with = _LINUX,
        link_libs = ":linux_link_libs_" + pair,
        resource_dir = resource_dir,
        runtime = runtime,
        toolchain = ":swift_cross_" + pair,
    )

    native.toolchain(
        name = name,
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:" + exec_cpu],
        target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:" + target_cpu],
        toolchain = ":swift_toolchain_" + pair,
        toolchain_type = "@rules_swift//toolchains:toolchain_type",
    )
