load(":native_runtime.bzl", "NativeRuntimeInfo")

"""Platform constraints and wheel tags for the native probe wheel."""

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
        ctx.actions.run(
            executable = "cmd.exe",
            arguments = ["/C", "copy /Y \"%s\" \"%s\" >NUL" % (file.path.replace("/", "\\"), copy.path.replace("/", "\\"))],
            inputs = [file],
            outputs = [copy],
            mnemonic = "CopyRuntimeFile",
            use_default_shell_env = True,
        )
        out.append(copy)
    return [DefaultInfo(files = depset(out))]

dx_native_probe_runtime_files = rule(
    implementation = _runtime_files_impl,
    doc = "Copies the files of a native_runtime next to the probe package so a wheel ships them.",
    attrs = {
        "runtime": attr.label(mandatory = True, providers = [NativeRuntimeInfo], doc = "The native_runtime whose files are copied."),
    },
)
