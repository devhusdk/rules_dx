"""Swift toolchain for Linux against a locked Ubuntu sysroot."""

load("@rules_cc//cc:cc_library.bzl", "cc_library")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_swift//swift:providers.bzl", "SwiftToolchainInfo")
load("@rules_swift//swift/toolchains:swift_toolchain.bzl", "swift_toolchain")
load("@rules_swift//swift/toolchains:swift_tools.bzl", "swift_tools")
load("//modules:versions.bzl", "SWIFT_VERSION")

def _runtime_files_impl(ctx):
    files = []
    for f in ctx.files.srcs:
        index = f.path.find("/usr/lib/")
        if index < 0:
            continue
        rel = f.path[index + 1:]
        if any([rel.startswith(d) for d in ctx.attr.dirs]) and any([rel.endswith(x) for x in ctx.attr.suffixes]):
            files.append(f)
    return [DefaultInfo(files = depset(files))]

_runtime_files = rule(
    implementation = _runtime_files_impl,
    attrs = {
        "dirs": attr.string_list(mandatory = True),
        "srcs": attr.label_list(allow_files = True),
        "suffixes": attr.string_list(mandatory = True),
    },
)

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
    archive = "@dx_swift_linux_" + cpu
    sysroot = "@dx_swift_sysroot_linux_" + cpu
    sysroot_root = Label(sysroot + "//:files").workspace_root
    tags = ["manual"]

    swift_tools(
        name = "tools_" + cpu,
        tags = tags,
        target_compatible_with = _LINUX,
        additional_inputs = [archive + "//:files", sysroot + "//:files"],
        swift_autolink_extract = archive + "//:usr/bin/swift-autolink-extract",
        swift_driver = archive + "//:usr/bin/swiftc",
        swift_symbolgraph_extract = archive + "//:usr/bin/swift-symbolgraph-extract",
    )

    _runtime_files(
        name = "dynamic_runtime_" + cpu,
        tags = tags,
        target_compatible_with = _LINUX,
        srcs = [archive + "//:files"],
        dirs = ["usr/lib/swift/linux/"],
        suffixes = [".so"],
    )

    _runtime_files(
        name = "static_runtime_" + cpu,
        tags = tags,
        target_compatible_with = _LINUX,
        srcs = [archive + "//:files"],
        dirs = [
            "usr/lib/swift/linux/",
            "usr/lib/swift_static/linux/",
        ],
        suffixes = [
            ".a",
            ".lnk",
            ".o",
        ],
    )

    swift_toolchain(
        name = "inner_toolchain_" + cpu,
        tags = tags,
        target_compatible_with = _LINUX,
        arch = cpu,
        dynamic_runtime = [":dynamic_runtime_" + cpu],
        env = {"LD_LIBRARY_PATH": sysroot_root + "/host_libs"},
        features = [
            "swift._supports_upcoming_features",
            "swift.no_embed_debug_module",
            "swift.use_autolink_extract",
            "swift.lld_gc_workaround",
            "swift.use_module_wrap",
            "swift.module_map_no_private_headers",
        ],
        os = "linux",
        parsed_version = SWIFT_VERSION,
        sdkroot = sysroot_root,
        static_runtime = [":static_runtime_" + cpu],
        swift_tools = ":tools_" + cpu,
        version_file = archive + "//:.swift-version",
    )

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
        toolchain = ":inner_toolchain_" + cpu,
    )

    native.toolchain(
        name = name,
        exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:" + cpu],
        target_compatible_with = ["@platforms//os:linux", "@platforms//cpu:" + cpu],
        toolchain = ":swift_toolchain_" + cpu,
        toolchain_type = "@rules_swift//toolchains:toolchain_type",
    )
