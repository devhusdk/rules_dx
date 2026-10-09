"""Local NDK-backed C++ toolchain repositories for Android native builds."""

load("//android/platforms:defs.bzl", "api_floor", "cxx_runtime_errors", "ndk_revision_errors", "rust_triples")

_SDK_ANCHOR = "source.properties"

_HOST_PREBUILT = "toolchains/llvm/prebuilt/linux-x86_64"

_QUALIFIED_HOST = "linux_x86_64"

def _prebuilt_for(ndk_root):
    return ndk_root + "/" + _HOST_PREBUILT

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
    errors.extend(ndk_revision_errors(revision))
    if api_level != 0 and api_level < api_floor():
        errors.append("android_ndk_toolchain: api_level " + str(api_level) + " is below " + str(api_floor()))
    errors.extend(cxx_runtime_errors(cxx_runtime))
    return errors

_TUPLE_BUILD = """
filegroup(
    name = "compiler_files_{tuple}",
    srcs = [
        "sdk/ndk/{revision}/" + "{prebuilt}/bin/clang",
    ] + glob(["sdk/ndk/{revision}/" + "{prebuilt}/lib/clang/{clang_version}/include/**"]),
)

filegroup(
    name = "linker_files_{tuple}",
    srcs = [
        "sdk/ndk/{revision}/" + "{prebuilt}/bin/clang",
        "sdk/ndk/{revision}/" + "{prebuilt}/bin/ld.lld",
    ] + glob([
        "sdk/ndk/{revision}/" + "{prebuilt}/lib/clang/{clang_version}/lib/linux/**",
        "sdk/ndk/{revision}/" + "{prebuilt}/{sysroot}/usr/lib/{triple}/**",
    ]),
)

filegroup(
    name = "all_files_{tuple}",
    srcs = [
        ":compiler_files_{tuple}",
        ":linker_files_{tuple}",
        ":ar_files",
    ],
)

ndk_cc_toolchain_config(
    name = "config_{tuple}",
    api_level = {api_level},
    clang_resource_dir = "{resource_dir}",
    clang_target = "{clang_target}",
    cpu = "{cpu}",
    cxx_include_dir = "{cxx_include_dir}",
    cxx_runtime = "{cxx_runtime}",
    ndk_prebuilt = "sdk/ndk/{revision}/{prebuilt}",
    ndk_sysroot = "{ndk_sysroot}",
    rust_triple = "{triple}",
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
    strip_files = ":empty",
    supports_param_files = False,
    toolchain_config = ":config_{tuple}",
    toolchain_identifier = "ndk-{tuple}-android",
)

toolchain(
    name = "ndk_cc_toolchain_{tuple}",
    target_compatible_with = {compatible_with},
    toolchain = ":cc_toolchain_{tuple}",
    toolchain_type = "@bazel_tools//tools/cpp:toolchain_type",
)
"""

_COMMON_BUILD = """
filegroup(name = "empty", srcs = [])

filegroup(
    name = "ar_files",
    srcs = ["sdk/ndk/{revision}/" + "{prebuilt}/bin/llvm-ar"],
)

filegroup(
    name = "llvm_readelf",
    srcs = ["sdk/ndk/{revision}/" + "{prebuilt}/bin/llvm-readelf"],
)
"""

_TUPLE_CPU = {"device": "arm64", "emulator": "x86_64"}

_TUPLE_TRIPLE = {
    "device": "aarch64-linux-android",
    "emulator": "x86_64-linux-android",
}

_TUPLE_CONSTRAINTS = {
    "device": '["@platforms//os:android", "@platforms//cpu:arm64"]',
    "emulator": '["@platforms//os:android", "@platforms//cpu:x86_64"]',
}

def _clang_version(ctx, clang):
    result = ctx.execute([clang, "--print-resource-dir"])
    if result.return_code != 0:
        fail("android_ndk_toolchain: '" + clang + "' cannot report its resource directory: " + result.stderr)
    fields = result.stdout.strip().split("/lib/clang/")
    if len(fields) != 2:
        fail("android_ndk_toolchain: '" + clang + "' reported an unexpected resource directory: " + result.stdout)
    return fields[1].split("/")[0]

def _android_ndk_toolchain_repo_impl(ctx):
    local_path = ctx.attr.local_path
    if local_path == "":
        local_path = ctx.os.environ.get("ANDROID_SDK_ROOT", "")
    revision = ctx.attr.ndk_revision
    if revision == "":
        revision = ctx.os.environ.get("ANDROID_NDK_REVISION", "")
    if local_path == "":
        ctx.file("BUILD.bazel", "package(default_visibility = [\"//visibility:public\"])\n")
        return
    failures = toolchain_repo_errors(local_path, revision, ctx.attr.api_level, ctx.attr.cxx_runtime)
    if len(failures) > 0:
        fail("; ".join(failures))
    if ctx.os.name != "linux" or ctx.os.arch != "amd64":
        fail("android_ndk_toolchain: qualified host is " + _QUALIFIED_HOST + ", this host is " +
             ctx.os.name + "_" + ctx.os.arch)
    for path in required_files(local_path, revision):
        if not ctx.path(path).exists:
            fail("android_ndk_toolchain: '" + path + "' is absent from the declared SDK")
    sdk = ctx.path(local_path)
    ctx.symlink(sdk, "sdk")
    prebuilt = _HOST_PREBUILT
    sysroot = "sysroot"
    clang = str(sdk) + "/ndk/" + revision + "/" + prebuilt + "/bin/clang"
    clang_version = _clang_version(ctx, clang)
    resource_dir = str(sdk) + "/ndk/" + revision + "/" + prebuilt + "/lib/clang/" + clang_version + "/include"
    cxx_include_dir = str(sdk) + "/ndk/" + revision + "/" + prebuilt + "/" + sysroot + "/usr/include/c++/v1"
    ndk_sysroot = str(sdk) + "/ndk/" + revision + "/" + prebuilt + "/" + sysroot
    api_level = ctx.attr.api_level if ctx.attr.api_level != 0 else api_floor()
    chunks = [
        'load("@rules_cc//cc:defs.bzl", "cc_toolchain")',
        'load("@rules_dx//android/toolchain:cc_toolchain_config.bzl", "ndk_cc_toolchain_config")',
        "",
        'package(default_visibility = ["//visibility:public"])',
        "",
        _COMMON_BUILD.format(revision = revision, prebuilt = prebuilt),
    ]
    for name in ["device", "emulator"]:
        triple = _TUPLE_TRIPLE[name]
        chunks.append(_TUPLE_BUILD.format(
            tuple = name,
            revision = revision,
            prebuilt = prebuilt,
            sysroot = sysroot,
            clang_version = clang_version,
            api_level = api_level,
            clang_target = triple + str(api_level),
            cpu = _TUPLE_CPU[name],
            triple = triple,
            cxx_runtime = ctx.attr.cxx_runtime,
            resource_dir = resource_dir,
            cxx_include_dir = cxx_include_dir,
            ndk_sysroot = ndk_sysroot,
            compatible_with = _TUPLE_CONSTRAINTS[name],
        ))
    ctx.file("BUILD.bazel", "\n".join(chunks))

android_ndk_toolchain_repo = repository_rule(
    implementation = _android_ndk_toolchain_repo_impl,
    environ = ["ANDROID_NDK_REVISION", "ANDROID_SDK_ROOT"],
    attrs = {
        "api_level": attr.int(default = 31),
        "cxx_runtime": attr.string(default = "shared"),
        "local_path": attr.string(default = ""),
        "ndk_revision": attr.string(default = ""),
    },
)
