"""Declares the rules_swift Windows toolchain over the unpacked swift.org installer."""

load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_swift//swift:providers.bzl", "SwiftToolchainInfo")
load("@rules_swift//swift/toolchains:swift_toolchain.bzl", "swift_toolchain")

_WINDOWS = ["@platforms//os:windows"]
_VERSION = "6.4.2"

def _with_link_inputs_impl(ctx):
    info = ctx.attr.toolchain[SwiftToolchainInfo]
    fields = {name: getattr(info, name) for name in dir(info) if name not in ("to_json", "to_proto")}
    link_inputs = CcInfo(
        linking_context = cc_common.create_linking_context(
            linker_inputs = depset([cc_common.create_linker_input(
                owner = ctx.label,
                additional_inputs = depset(ctx.files.link_inputs),
            )]),
        ),
    )
    implicit = info.implicit_deps_providers
    fields["implicit_deps_providers"] = struct(
        cc_infos = implicit.cc_infos + [link_inputs],
        swift_infos = implicit.swift_infos,
    )
    merged = SwiftToolchainInfo(**fields)
    return [
        platform_common.ToolchainInfo(swift_toolchain = merged),
        merged,
    ]

_with_link_inputs = rule(
    implementation = _with_link_inputs_impl,
    attrs = {
        "link_inputs": attr.label(mandatory = True, allow_files = True),
        "toolchain": attr.label(mandatory = True, providers = [SwiftToolchainInfo]),
    },
)

def swift_windows_toolchain(name, cpu):
    """Declares the Swift toolchain for one Windows cpu."""
    repo = "@dx_swift_windows_" + cpu
    root = Label(repo + "//:.swift-version").workspace_root
    runtime = (root + "/runtime/usr/bin").replace("/", "\\")

    swift_toolchain(
        name = name + "_base",
        arch = cpu,
        env = {"PATH": runtime, "Path": runtime},
        os = "windows",
        parsed_version = _VERSION,
        sdkroot = root + "/platform/Developer/SDKs/Windows.sdk",
        swift_tools = repo + "//:tools",
        tags = ["manual"],
        target_compatible_with = _WINDOWS,
        tool_executable_suffix = ".exe",
        version_file = repo + "//:.swift-version",
        xctest_version = _VERSION,
        features = [
            "swift._supports_upcoming_features",
            "swift.no_embed_debug_module",
            "swift.module_map_no_private_headers",
        ],
    )

    _with_link_inputs(
        name = name,
        link_inputs = repo + "//:link_inputs",
        tags = ["manual"],
        target_compatible_with = _WINDOWS,
        toolchain = ":" + name + "_base",
    )
