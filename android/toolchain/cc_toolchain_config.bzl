"""Minimal NDK clang toolchain configuration for one Android tuple."""

load("@rules_cc//cc:cc_toolchain_config_lib.bzl", "feature", "flag_group", "flag_set", "tool_path")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")

def _ndk_cc_toolchain_config_impl(ctx):
    prebuilt = ctx.attr.ndk_prebuilt
    target = ctx.attr.clang_target
    toolchain_identifier = "ndk-" + ctx.attr.tuple + "-android"
    tool_paths = [
        tool_path(name = "gcc", path = prebuilt + "/bin/clang"),
        tool_path(name = "ld", path = prebuilt + "/bin/ld.lld"),
        tool_path(name = "ar", path = prebuilt + "/bin/llvm-ar"),
        tool_path(name = "cpp", path = prebuilt + "/bin/clang++"),
        tool_path(name = "gcov", path = prebuilt + "/bin/llvm-cov"),
        tool_path(name = "nm", path = prebuilt + "/bin/llvm-nm"),
        tool_path(name = "objdump", path = prebuilt + "/bin/llvm-objdump"),
        tool_path(name = "strip", path = prebuilt + "/bin/llvm-strip"),
    ]
    compile_flags = ["--target=" + target]
    link_flags = ["--target=" + target]
    if ctx.attr.cxx_runtime == "shared":
        link_flags.extend(["-stdlib=libc++", "-lc++_shared"])
    elif ctx.attr.cxx_runtime == "static":
        link_flags.extend(["-stdlib=libc++", "-lc++_static"])
    else:
        link_flags.append("-nostdlib++")
    features = [
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
    return cc_common.create_cc_toolchain_config_info(
        ctx = ctx,
        toolchain_identifier = toolchain_identifier,
        host_system_name = "local",
        target_system_name = "android",
        target_cpu = ctx.attr.cpu,
        target_libc = "bionic",
        compiler = "clang",
        abi_version = "android",
        abi_libc_version = str(ctx.attr.api_level),
        tool_paths = tool_paths,
        cxx_builtin_include_directories = cxx_builtin_include_directories,
        features = features,
        builtin_sysroot = sysroot,
    )

ndk_cc_toolchain_config = rule(
    implementation = _ndk_cc_toolchain_config_impl,
    attrs = {
        "api_level": attr.int(mandatory = True),
        "clang_resource_dir": attr.string(mandatory = True),
        "clang_target": attr.string(mandatory = True),
        "cpu": attr.string(mandatory = True),
        "cxx_include_dir": attr.string(mandatory = True),
        "cxx_runtime": attr.string(mandatory = True),
        "ndk_prebuilt": attr.string(mandatory = True),
        "ndk_sysroot": attr.string(mandatory = True),
        "rust_triple": attr.string(mandatory = True),
        "tuple": attr.string(mandatory = True),
    },
)
