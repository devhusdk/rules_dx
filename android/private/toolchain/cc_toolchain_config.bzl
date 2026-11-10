"""Minimal NDK clang toolchain configuration for one Android tuple."""

load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("@rules_cc//cc:action_names.bzl", "ACTION_NAMES")
load("@rules_cc//cc:cc_toolchain_config_lib.bzl", "action_config", "feature", "flag_group", "flag_set", "tool", "variable_with_value")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")

NdkToolchainLevelInfo = provider(
    doc = "API level and clang target an NDK toolchain configuration uses.",
    fields = ["api_level", "clang_target"],
)

def _ndk_cc_toolchain_config_impl(ctx):
    selected = ctx.attr._api_level[BuildSettingInfo].value
    api_level = selected if selected != 0 else ctx.attr.api_level
    target = ctx.attr.rust_triple + str(api_level) if selected != 0 else ctx.attr.clang_target
    toolchain_identifier = "ndk-" + ctx.attr.tuple + "-android"
    clang = tool(tool = ctx.file.clang)
    action_configs = [
        action_config(action_name = name, enabled = True, tools = [clang])
        for name in [
            ACTION_NAMES.c_compile,
            ACTION_NAMES.cpp_compile,
            ACTION_NAMES.assemble,
            ACTION_NAMES.preprocess_assemble,
            ACTION_NAMES.cpp_header_parsing,
            ACTION_NAMES.cpp_module_compile,
            ACTION_NAMES.cpp_module_codegen,
            ACTION_NAMES.linkstamp_compile,
            ACTION_NAMES.cpp_link_executable,
            ACTION_NAMES.cpp_link_dynamic_library,
            ACTION_NAMES.cpp_link_nodeps_dynamic_library,
        ]
    ] + [
        action_config(action_name = ACTION_NAMES.cpp_link_static_library, enabled = True, tools = [tool(tool = ctx.file.ar)]),
        action_config(action_name = ACTION_NAMES.strip, enabled = True, tools = [tool(tool = ctx.file.strip)]),
    ]
    resource_dir = ctx.attr.clang_resource_dir.removesuffix("/include")
    compile_flags = ["--target=" + target, "-resource-dir=" + resource_dir]
    link_flags = ["--target=" + target]
    if ctx.attr.cxx_runtime == "shared":
        link_flags.extend(["-stdlib=libc++", "-lc++_shared"])
    elif ctx.attr.cxx_runtime == "static":
        link_flags.extend(["-stdlib=libc++", "-lc++_static"])
    else:
        link_flags.append("-nostdlib++")
    features = [
        feature(
            name = "archiver_flags",
            enabled = True,
            flag_sets = [
                flag_set(
                    actions = [ACTION_NAMES.cpp_link_static_library],
                    flag_groups = [
                        flag_group(flags = ["rcsD"]),
                        flag_group(flags = ["%{output_execpath}"], expand_if_available = "output_execpath"),
                        flag_group(
                            iterate_over = "libraries_to_link",
                            flag_groups = [
                                flag_group(
                                    flags = ["%{libraries_to_link.name}"],
                                    expand_if_equal = variable_with_value(name = "libraries_to_link.type", value = "object_file"),
                                ),
                                flag_group(
                                    iterate_over = "libraries_to_link.object_files",
                                    flags = ["%{libraries_to_link.object_files}"],
                                    expand_if_equal = variable_with_value(name = "libraries_to_link.type", value = "object_file_group"),
                                ),
                            ],
                            expand_if_available = "libraries_to_link",
                        ),
                    ],
                ),
            ],
        ),
        feature(
            name = "default_compile_flags",
            enabled = True,
            flag_sets = [
                flag_set(
                    actions = [
                        "c-compile",
                        "c++-compile",
                        "c++-header-parsing",
                        "c++-module-compile",
                        "c++-module-codegen",
                    ],
                    flag_groups = [flag_group(flags = compile_flags)],
                ),
            ],
        ),
        feature(
            name = "default_link_flags",
            enabled = True,
            flag_sets = [
                flag_set(
                    actions = [
                        "c++-link-executable",
                        "c++-link-dynamic-library",
                        "c++-link-nodeps-dynamic-library",
                    ],
                    flag_groups = [flag_group(flags = link_flags)],
                ),
            ],
        ),
    ]
    sysroot = ctx.attr.ndk_sysroot
    cxx_builtin_include_directories = [
        sysroot + "/usr/include",
        sysroot + "/usr/include/" + ctx.attr.rust_triple,
        ctx.attr.clang_resource_dir,
        ctx.attr.cxx_include_dir,
    ]
    config = cc_common.create_cc_toolchain_config_info(
        ctx = ctx,
        toolchain_identifier = toolchain_identifier,
        host_system_name = "local",
        target_system_name = "android",
        target_cpu = ctx.attr.cpu,
        target_libc = "bionic",
        compiler = "clang",
        abi_version = "android",
        abi_libc_version = str(api_level),
        tool_paths = [],
        action_configs = action_configs,
        cxx_builtin_include_directories = cxx_builtin_include_directories,
        features = features,
        builtin_sysroot = sysroot,
    )
    return [config, NdkToolchainLevelInfo(api_level = api_level, clang_target = target)]

ndk_cc_toolchain_config = rule(
    implementation = _ndk_cc_toolchain_config_impl,
    doc = "NDK clang toolchain configuration for one Android tuple.",
    attrs = {
        "ar": attr.label(
            doc = "NDK llvm-ar used to archive static libraries.",
            allow_single_file = True,
            cfg = "exec",
            mandatory = True,
        ),
        "api_level": attr.int(
            doc = "Android API level used when no target selects one.",
            mandatory = True,
        ),
        "_api_level": attr.label(
            default = Label("//android/platforms:api_level"),
        ),
        "clang": attr.label(
            doc = "NDK clang used to compile, assemble and link.",
            allow_single_file = True,
            cfg = "exec",
            mandatory = True,
        ),
        "clang_resource_dir": attr.string(
            doc = "Clang resource directory added to builtin include paths.",
            mandatory = True,
        ),
        "clang_target": attr.string(
            doc = "Clang target triple passed to every compile and link.",
            mandatory = True,
        ),
        "cpu": attr.string(doc = "Target CPU name reported to the toolchain.", mandatory = True),
        "cxx_include_dir": attr.string(
            doc = "C++ standard library include directory added to builtin includes.",
            mandatory = True,
        ),
        "cxx_runtime": attr.string(
            doc = "C++ runtime linkage, shared or static; any other value disables it.",
            mandatory = True,
        ),
        "ndk_sysroot": attr.string(
            doc = "NDK sysroot directory used as the builtin sysroot.",
            mandatory = True,
        ),
        "rust_triple": attr.string(
            doc = "Rust target triple used in the sysroot include path.",
            mandatory = True,
        ),
        "strip": attr.label(
            doc = "NDK llvm-strip used to strip binaries.",
            allow_single_file = True,
            cfg = "exec",
            mandatory = True,
        ),
        "tuple": attr.string(
            doc = "Android ABI tuple that names the toolchain identifier.",
            mandatory = True,
        ),
    },
)
