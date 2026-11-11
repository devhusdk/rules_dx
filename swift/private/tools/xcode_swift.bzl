"""The macOS `swift` command: a native launcher that runs the Xcode `swift` through `apple_ops`."""

load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "write_native_launcher")
load("//core:rlocation.bzl", "rlocation_path")

def _swift_xcode_launcher_impl(ctx):
    xcode = ctx.attr._xcode_config[apple_common.XcodeVersionConfig].xcode_version()
    launcher, _, launcher_runfiles = write_native_launcher(ctx, "\n".join([
        "program=" + rlocation_path(ctx, ctx.executable._ops),
        "arg=swift",
        "env=DX_XCODE_VERSION\t" + (str(xcode) if xcode else ""),
    ]) + "\n")
    return DefaultInfo(
        executable = launcher,
        runfiles = launcher_runfiles.merge(ctx.attr._ops[DefaultInfo].default_runfiles),
    )

swift_xcode_launcher = rule(
    implementation = _swift_xcode_launcher_impl,
    doc = "A launcher that runs `swift` from the configured Xcode.",
    attrs = {
        "_ops": attr.label(
            default = Label("//apple/private/tools/ops:apple_ops"),
            executable = True,
            cfg = "target",
        ),
        "_xcode_config": attr.label(
            default = configuration_field(name = "xcode_config_label", fragment = "apple"),
        ),
    } | NATIVE_LAUNCHER_ATTRS,
    executable = True,
    fragments = ["apple"],
)
