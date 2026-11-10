"""Local NDK-backed C++ toolchain repositories for Android native builds."""

load("//android/platforms:defs.bzl", "api_floor", "constraint_list", "rust_triples")
load("//android/platforms:validate.bzl", "android_cxx_runtime_errors", "android_ndk_revision_errors")
load("//android/private/sdk:repos.bzl", "ANDROID_NDK_CLANG_VERSION", "ANDROID_NDK_REVISION", "NDK_PREBUILT", "ndk_repository_name", "ndk_toolchain_build")

_SDK_ANCHOR = "source.properties"

_QUALIFIED_HOST = "linux_x86_64"

def _prebuilt_for(ndk_root):
    return ndk_root + "/" + NDK_PREBUILT

def _sysroot_for(ndk_root):
    return _prebuilt_for(ndk_root) + "/sysroot"

def required_files(local_path, revision):
    """Returns the SDK paths one toolchain repository selection must provide."""
    ndk_root = local_path + "/ndk/" + revision
    prebuilt = _prebuilt_for(ndk_root)
    sysroot = _sysroot_for(ndk_root)
    required = [
        ndk_root + "/" + _SDK_ANCHOR,
        prebuilt + "/bin/clang",
        prebuilt + "/bin/llvm-ar",
        prebuilt + "/bin/ld.lld",
        sysroot + "/usr/include/stdlib.h",
        sysroot + "/usr/include/c++/v1/vector",
    ]
    for triple in rust_triples():
        required.append(sysroot + "/usr/lib/" + triple)
    return required

def toolchain_repo_errors(local_path, revision, api_level, cxx_runtime):
    """Returns one error string per rejected toolchain repository selection."""
    errors = []
    if local_path == "":
        errors.append("android_ndk_toolchain: local_path names the installed Android SDK")
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

def toolchain_build(ndk_label, exec_root, api_level, cxx_runtime, local_root = ""):
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
    if local_root != "":
        chunks.append(ndk_toolchain_build(local_root))
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
    if ctx.attr.local_path == "":
        failures = toolchain_repo_errors("pinned", ANDROID_NDK_REVISION, api_level, ctx.attr.cxx_runtime)
        if len(failures) > 0:
            fail("; ".join(failures))
        ndk = Label("@" + ndk_repository_name(_QUALIFIED_HOST) + "//:ndk_anchor")
        ctx.file("BUILD.bazel", toolchain_build(
            str(ndk).removesuffix(":ndk_anchor"),
            ndk.workspace_root + "/extracted",
            api_level,
            ctx.attr.cxx_runtime,
        ))
        return
    revision = ctx.attr.ndk_revision or ANDROID_NDK_REVISION
    failures = toolchain_repo_errors(ctx.attr.local_path, revision, ctx.attr.api_level, ctx.attr.cxx_runtime)
    if len(failures) > 0:
        fail("; ".join(failures))
    if ctx.os.name != "linux" or ctx.os.arch != "amd64":
        fail("android_ndk_toolchain: qualified host is " + _QUALIFIED_HOST + ", this host is " +
             ctx.os.name + "_" + ctx.os.arch)
    for path in required_files(ctx.attr.local_path, revision):
        if not ctx.path(path).exists:
            fail("android_ndk_toolchain: '" + path + "' is absent from the declared SDK")
    ctx.symlink(ctx.path(ctx.attr.local_path), "sdk")
    local_root = "sdk/ndk/" + revision
    ctx.file("BUILD.bazel", toolchain_build(
        "",
        "external/" + ctx.name + "/" + local_root,
        api_level,
        ctx.attr.cxx_runtime,
        local_root,
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
        "local_path": attr.string(
            doc = "Installed Android SDK root to use instead of the pinned NDK repository.",
            default = "",
        ),
        "ndk_revision": attr.string(
            doc = "NDK revision under local_path, defaulting to the pinned one.",
            default = "",
        ),
    },
)
