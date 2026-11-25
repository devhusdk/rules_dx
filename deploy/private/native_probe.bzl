"""Platform constraints and wheel tags for the native probe wheel."""

load("//core/coreutils:defs.bzl", "COREUTILS_TOOLCHAIN_TYPE", "coreutils_copy")
load(":native_runtime.bzl", "NativeRuntimeInfo")

NATIVE_PROBE_COMPATIBLE = select({
    "@platforms//os:linux": ["@platforms//cpu:x86_64"],
    "@platforms//os:windows": ["@platforms//cpu:x86_64"],
    "//conditions:default": ["@platforms//:incompatible"],
})

NATIVE_PROBE_PLATFORM = select({
    "@platforms//os:windows": "win_amd64",
    "//conditions:default": "linux_x86_64",
})

def _runtime_files_impl(ctx):
    out = []
    for file in ctx.attr.runtime[NativeRuntimeInfo].files:
        copy = ctx.actions.declare_file(file.basename)
        coreutils_copy(ctx, file, copy, "CopyRuntimeFile")
        out.append(copy)
    return [DefaultInfo(files = depset(out))]

dx_native_probe_runtime_files = rule(
    implementation = _runtime_files_impl,
    doc = "Copies the files of a native_runtime next to the probe package so a wheel ships them.",
    toolchains = [COREUTILS_TOOLCHAIN_TYPE],
    attrs = {
        "runtime": attr.label(mandatory = True, providers = [NativeRuntimeInfo], doc = "The native_runtime whose files are copied."),
    },
)
