"""NDK-backed C++ toolchain repositories for Android native builds."""

load("//android/platforms:defs.bzl", "api_floor", "constraint_list")
load("//android/platforms:validate.bzl", "android_cxx_runtime_errors", "android_ndk_revision_errors")
load("//android/private/sdk:repos.bzl", "ANDROID_NDK_CLANG_VERSION", "ANDROID_NDK_REVISION", "NDK_PREBUILT", "ndk_repository_name")

_QUALIFIED_HOST = "linux_x86_64"

def toolchain_repo_errors(revision, api_level, cxx_runtime):
    """Returns one error string per rejected toolchain repository selection."""
    errors = []
    errors.extend(android_ndk_revision_errors(revision))
    if api_level != 0 and api_level < api_floor():
        errors.append("android_ndk_toolchain: api_level " + str(api_level) + " is below " + str(api_floor()))
    errors.extend(android_cxx_runtime_errors(cxx_runtime))
    return errors

_TUPLE_BUILD = """
filegroup(
    name = "compiler_files_{tuple}",
    srcs = ["{ndk}:clang", "{ndk}:compiler_files", ":native_tools"],
)

filegroup(
    name = "linker_files_{tuple}",
    srcs = ["{ndk}:clang", "{ndk}:ld_lld", "{ndk}:linker_files_{triple}", ":native_tools"],
)

filegroup(
    name = "all_files_{tuple}",
    srcs = [
        ":compiler_files_{tuple}",
        ":linker_files_{tuple}",
        ":ar_files",
        ":strip_files",
    ],
)

ndk_cc_toolchain_config(
    name = "config_{tuple}",
    api_level = {api_level},
    ar = "{ndk}:llvm_ar",
    clang = "{ndk}:clang",
    clang_resource_dir = "{resource_dir}",
    clang_target = "{clang_target}",
    cpu = "{cpu}",
    cxx_include_dir = "{cxx_include_dir}",
    cxx_runtime = "{cxx_runtime}",
    ndk_sysroot = "{ndk_sysroot}",
    rust_triple = "{triple}",
    strip = "{ndk}:llvm_strip",
    tuple = "{tuple}",
)

cc_toolchain(
    name = "cc_toolchain_{tuple}",
    all_files = ":all_files_{tuple}",
    ar_files = ":ar_files",
    compiler_files = ":compiler_files_{tuple}",
    dwp_files = ":empty",
    linker_files = ":linker_files_{tuple}",
    objcopy_files = ":empty",
    strip_files = ":strip_files",
    supports_param_files = False,
    toolchain_config = ":config_{tuple}",
    toolchain_identifier = "ndk-{tuple}-android",
)

toolchain(
    name = "ndk_cc_toolchain_{tuple}",
    exec_compatible_with = ["@platforms//os:linux", "@platforms//cpu:x86_64"],
    target_compatible_with = {compatible_with},
    toolchain = ":cc_toolchain_{tuple}",
    toolchain_type = "@bazel_tools//tools/cpp:toolchain_type",
)
"""

_COMMON_BUILD = """
filegroup(name = "empty", srcs = [])

filegroup(
    name = "ar_files",
    srcs = ["{ndk}:llvm_ar", ":native_tools"],
)

filegroup(
    name = "llvm_readelf",
    srcs = ["{ndk}:llvm_readelf"],
)

filegroup(
    name = "strip_files",
    srcs = ["{ndk}:llvm_strip", ":native_tools"],
)
"""

_TUPLE_CPU = {"device": "arm64", "emulator": "x86_64"}

_TUPLE_TRIPLE = {
    "device": "aarch64-linux-android",
    "emulator": "x86_64-linux-android",
}

_TUPLE_CONSTRAINTS = {
    name: '["' + '", "'.join(constraint_list(name)) + '"]'
    for name in ["device", "emulator"]
}

def toolchain_build(ndk_label, exec_root, api_level, cxx_runtime):
    """Returns the BUILD text declaring both NDK cc toolchains."""
    prebuilt = exec_root + "/" + NDK_PREBUILT
    sysroot = prebuilt + "/sysroot"
    chunks = [
        'load("' + str(Label("@rules_cc//cc:defs.bzl")) + '", "cc_toolchain")',
        'load("' + str(Label("//android/private/toolchain:cc_toolchain_config.bzl")) + '", "ndk_cc_toolchain_config")',
        "",
        'package(default_visibility = ["//visibility:public"])',
        "",
        _COMMON_BUILD.format(ndk = ndk_label),
    ]
    chunks.append('filegroup(name = "native_tools", srcs = [])')
    for name in ["device", "emulator"]:
        triple = _TUPLE_TRIPLE[name]
        chunks.append(_TUPLE_BUILD.format(
            ndk = ndk_label,
            tuple = name,
            api_level = api_level,
            clang_target = triple + str(api_level),
            cpu = _TUPLE_CPU[name],
            triple = triple,
            cxx_runtime = cxx_runtime,
            resource_dir = prebuilt + "/lib/clang/" + ANDROID_NDK_CLANG_VERSION + "/include",
            cxx_include_dir = sysroot + "/usr/include/c++/v1",
            ndk_sysroot = sysroot,
            compatible_with = _TUPLE_CONSTRAINTS[name],
        ))
    return "\n".join(chunks)

def _android_ndk_toolchain_repo_impl(ctx):
    api_level = ctx.attr.api_level if ctx.attr.api_level != 0 else api_floor()
    failures = toolchain_repo_errors(ANDROID_NDK_REVISION, api_level, ctx.attr.cxx_runtime)
    if len(failures) > 0:
        fail("; ".join(failures))
    ndk = Label("@" + ndk_repository_name(_QUALIFIED_HOST) + "//:ndk_anchor")
    ctx.file("BUILD.bazel", toolchain_build(
        str(ndk).removesuffix(":ndk_anchor"),
        ndk.workspace_root + "/extracted",
        api_level,
        ctx.attr.cxx_runtime,
    ))

android_ndk_toolchain_repo = repository_rule(
    implementation = _android_ndk_toolchain_repo_impl,
    attrs = {
        "api_level": attr.int(
            doc = "Android API level used when a target does not set one, or zero for the floor.",
            default = 31,
        ),
        "cxx_runtime": attr.string(
            doc = "C++ runtime linkage for the NDK toolchain, shared by default.",
            default = "shared",
        ),
    },
)
