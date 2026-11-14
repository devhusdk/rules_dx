"""NDK-backed C++ toolchain repositories for Android native builds."""

load("//android/private/platforms:defs.bzl", "api_floor", "constraint_list")
load("//android/private/platforms:validate.bzl", "android_cxx_runtime_errors", "android_ndk_revision_errors")
load("//android/private/sdk:repos.bzl", "ANDROID_NDK_CLANG_VERSION", "ANDROID_NDK_REVISION", "NDK_ARTIFACTS", "NDK_PREBUILT", "PLATFORM_CONSTRAINTS", "ndk_repository_name")

def toolchain_repo_errors(revision, api_level, cxx_runtime):
    """Returns one error string per rejected toolchain repository selection."""
    errors = []
    errors.extend(android_ndk_revision_errors(revision))
    if api_level != 0 and api_level < api_floor():
        errors.append("android_ndk_toolchain: api_level " + str(api_level) + " is below " + str(api_floor()))
    errors.extend(android_cxx_runtime_errors(cxx_runtime))
    return errors

_TOOLCHAINS_ON = str(Label("//config:toolchains_on"))

_TUPLE_BUILD = """
filegroup(
    name = "compiler_files_{tuple}_{host}",
    srcs = ["{ndk}:clang", "{ndk}:compiler_files", ":native_tools_{host}"],
)

filegroup(
    name = "linker_files_{tuple}_{host}",
    srcs = ["{ndk}:clang", "{ndk}:ld_lld", "{ndk}:runtime_libs", "{ndk}:linker_files_{triple}", ":native_tools_{host}"],
)

filegroup(
    name = "all_files_{tuple}_{host}",
    srcs = [
        ":compiler_files_{tuple}_{host}",
        ":linker_files_{tuple}_{host}",
        ":ar_files_{host}",
        ":strip_files_{host}",
    ],
)

ndk_cc_toolchain_config(
    name = "config_{tuple}_{host}",
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
    name = "cc_toolchain_{tuple}_{host}",
    all_files = ":all_files_{tuple}_{host}",
    ar_files = ":ar_files_{host}",
    compiler_files = ":compiler_files_{tuple}_{host}",
    dwp_files = ":empty",
    linker_files = ":linker_files_{tuple}_{host}",
    objcopy_files = ":empty",
    strip_files = ":strip_files_{host}",
    supports_param_files = False,
    toolchain_config = ":config_{tuple}_{host}",
    toolchain_identifier = "ndk-{tuple}-android",
)

toolchain(
    name = "ndk_cc_toolchain_{tuple}_{host}",
    exec_compatible_with = {exec_compatible_with},
    target_compatible_with = {compatible_with},
    target_settings = ["{toolchains_on}"],
    toolchain = ":cc_toolchain_{tuple}_{host}",
    toolchain_type = "@bazel_tools//tools/cpp:toolchain_type",
)
"""

_HOST_BUILD = """
filegroup(name = "native_tools_{host}", srcs = [])

filegroup(
    name = "ar_files_{host}",
    srcs = ["{ndk}:llvm_ar", ":native_tools_{host}"],
)

config_setting(
    name = "{host}",
    constraint_values = {exec_compatible_with},
)

filegroup(
    name = "strip_files_{host}",
    srcs = ["{ndk}:llvm_strip", ":native_tools_{host}"],
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

def _list(values):
    return '["' + '", "'.join(values) + '"]'

def toolchain_build(ndks, api_level, cxx_runtime):
    """Returns the BUILD text declaring both NDK cc toolchains for every pinned exec host."""
    chunks = [
        'load("' + str(Label("@rules_cc//cc:defs.bzl")) + '", "cc_toolchain")',
        'load("' + str(Label("//android/private/toolchain:cc_toolchain_config.bzl")) + '", "ndk_cc_toolchain_config")',
        "",
        'package(default_visibility = ["//visibility:public"])',
        "",
        'filegroup(name = "empty", srcs = [])',
    ]
    for host in sorted(ndks.keys()):
        ndk = ndks[host]
        prebuilt = ndk.exec_root + "/" + NDK_PREBUILT[host]
        sysroot = prebuilt + "/sysroot"
        chunks.append(_HOST_BUILD.format(ndk = ndk.label, host = host, exec_compatible_with = _list(PLATFORM_CONSTRAINTS[host])))
        for name in ["device", "emulator"]:
            triple = _TUPLE_TRIPLE[name]
            chunks.append(_TUPLE_BUILD.format(
                ndk = ndk.label,
                host = host,
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
                exec_compatible_with = _list(PLATFORM_CONSTRAINTS[host]),
                toolchains_on = _TOOLCHAINS_ON,
            ))
    chunks.extend([
        "alias(",
        '    name = "llvm_readelf",',
        "    actual = select({",
    ] + ['        ":%s": "%s:llvm_readelf",' % (host, ndks[host].label) for host in sorted(ndks.keys())] + [
        "    }),",
        ")",
    ])
    return "\n".join(chunks)

def _android_ndk_toolchain_repo_impl(ctx):
    api_level = ctx.attr.api_level if ctx.attr.api_level != 0 else api_floor()
    failures = toolchain_repo_errors(ANDROID_NDK_REVISION, api_level, ctx.attr.cxx_runtime)
    if len(failures) > 0:
        fail("; ".join(failures))
    ndks = {}
    for host in sorted(NDK_ARTIFACTS.keys()):
        ndk = Label("@" + ndk_repository_name(host) + "//:ndk_anchor")
        ndks[host] = struct(
            label = str(ndk).removesuffix(":ndk_anchor"),
            exec_root = ndk.workspace_root + "/extracted",
        )
    ctx.file("BUILD.bazel", toolchain_build(ndks, api_level, ctx.attr.cxx_runtime))

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
